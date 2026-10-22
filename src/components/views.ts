import { BarChart3, CalendarDays, CalendarRange, FolderKanban, ListChecks, Sun, Tag, type LucideIcon } from "lucide-react";
import type { View } from "../state/store";

/** Every view; `view.<id>` shows one, bound to ⌘ and its position. The sidebar lists the pages, the calendar the rest. */
export const VIEWS: { id: View; label: string; icon: LucideIcon }[] = [
  { id: "day", label: "Day", icon: Sun },
  { id: "week", label: "Week", icon: CalendarRange },
  { id: "month", label: "Month", icon: CalendarDays },
  { id: "reports", label: "Reports", icon: BarChart3 },
  { id: "projects", label: "Projects", icon: FolderKanban },
  { id: "tags", label: "Tags", icon: Tag },
  { id: "rules", label: "Rules", icon: ListChecks },
];
