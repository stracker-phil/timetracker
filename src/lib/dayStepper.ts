import { api, type PeriodUnit } from "../api";
import { fail } from "../state/requests";

let queue: Promise<void> = Promise.resolve();

/**
 * Steps the day `day` returns (read when its turn comes) by `by` units through Rust (`calendarStep`) and hands
 * the result to `use`. Steps run one after the other, so quick presses add up; a failure toasts.
 */
export function stepThroughRust(unit: PeriodUnit, day: () => string, by: number, use: (day: string) => unknown) {
  queue = queue.then(() => api.calendarStep(unit, day(), by)).then(
    (stepped) => void use(stepped),
    (e: unknown) => void fail(e, "Could not change the date"),
  );
}

/** A stepper for the day `current` returns, which shows each step with `show`. */
export function dayStepper(current: () => string, show: (day: string) => void) {
  return (unit: PeriodUnit, by: number) => stepThroughRust(unit, current, by, show);
}
