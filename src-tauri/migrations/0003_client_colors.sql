-- A client has a color; its projects use that hue in a shade of their own (`projects.color` follows the client).
ALTER TABLE clients ADD COLUMN color TEXT NOT NULL DEFAULT 'blue';
ALTER TABLE projects ADD COLUMN shade INTEGER NOT NULL DEFAULT 4;

UPDATE clients SET color = (
  SELECT p.color FROM projects p WHERE p.client_id = clients.id
  GROUP BY p.color ORDER BY count(*) DESC, p.color LIMIT 1)
WHERE EXISTS (SELECT 1 FROM projects p WHERE p.client_id = clients.id);

UPDATE projects SET color = (SELECT c.color FROM clients c WHERE c.id = projects.client_id)
WHERE client_id IS NOT NULL;

-- Siblings get distinct shades in the order `projects::SHADE_ORDER` hands them out.
WITH ranked AS (
  SELECT id, (ROW_NUMBER() OVER (PARTITION BY client_id ORDER BY created_at, id) - 1) % 9 AS n FROM projects)
UPDATE projects SET shade = (
  SELECT CASE n WHEN 0 THEN 4 WHEN 1 THEN 1 WHEN 2 THEN 7 WHEN 3 THEN 2 WHEN 4 THEN 6
                WHEN 5 THEN 0 WHEN 6 THEN 8 WHEN 7 THEN 3 ELSE 5 END
  FROM ranked WHERE ranked.id = projects.id);
