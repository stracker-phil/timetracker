# Stable controls

UI changes follow [docs/ux.md → Stable controls](../../docs/ux.md#stable-controls):

- A click never changes what is under the cursor; clicking the same spot again does the same thing again
  (a Settings switch switches back, the sidebar status's Pause becomes Play in the same slot).
- Lists and pickers keep their order while open; toggling an item changes only its switch or box (View
  options' App lanes).
- Toggles keep their size (state by color, not by an added icon or text); optional buttons keep their slot
  (the Day toolbar's View options and ⋯ never move when the totals change).
- What appears fades in; what moves glides (`useGlide`, `useHeightGlide` in `lib/motion.ts`); durations come
  from the motion tokens only. Timeline lanes and toolbars change width with the window only, never on a click.
- Every user-scrollable box carries `.scroller` and starts below its title row (docs/ux.md → Scrollbars).

Before finishing a UI change, click every new or changed control twice in the debug app and check that
nothing beside it moved.
