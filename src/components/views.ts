import { BarChart3, CalendarDays, CalendarRange, FolderKanban, ListChecks, Sun, type LucideIcon } from "lucide-react";
import type { View } from "../state/store";

/** The main views in sidebar order; `view.<id>` shows one, bound to ⌘ and its position. */
export const VIEWS: { id: View; label: string; icon: LucideIcon }[] = [
  { id: "day", label: "Day", icon: Sun },
  { id: "week", label: "Week", icon: CalendarRange },
  { id: "month", label: "Month", icon: CalendarDays },
  { id: "reports", label: "Reports", icon: BarChart3 },
  { id: "projects", label: "Projects", icon: FolderKanban },
  { id: "rules", label: "Rules", icon: ListChecks },
];
