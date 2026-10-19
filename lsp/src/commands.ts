// Editor commands → hvolt CLI invocations.
//
// Kept as a pure mapping so it can be tested without spawning anything; the server
// adds cwd, execution and reporting.

export interface CommandSpec {
  args: string[];
  /** Streamed commands keep running; one-shot ones resolve when the CLI exits. */
  longRunning?: boolean;
}

export interface CommandRequest {
  /** Device serial or host:port, when the CLI needs one. */
  device?: string;
  /** Output path for screenshot. */
  out?: string;
  /** Sign locally instead of through the (private) AGC backend. */
  local?: boolean;
  /** Seconds of log to collect before returning. */
  seconds?: number;
}

const SPECS: Record<string, (request: CommandRequest) => CommandSpec> = {
  "hvolt.build": () => ({ args: ["build"] }),
  "hvolt.assemble": () => ({ args: ["assemble"] }),
  "hvolt.sign": (request) => ({ args: request.local === false ? ["sign"] : ["sign", "--local"] }),
  "hvolt.push": (request) => ({
    args: ["push", ...(request.device ? ["--device", request.device] : [])],
  }),
  "hvolt.run": (request) => ({ args: ["run", ...(request.device ? ["--device", request.device] : [])] }),
  "hvolt.screenshot": (request) => ({
    args: ["screenshot", ...(request.out ? ["--out", request.out] : []), ...(request.device ? ["--device", request.device] : [])],
  }),
  "hvolt.log": (request) => ({
    // The device log never ends by itself; the caller collects for a fixed window.
    args: ["log", ...(request.device ? ["--device", request.device] : [])],
    longRunning: true,
  }),
};

export function commandSpec(command: string, request: CommandRequest = {}): CommandSpec | null {
  const factory = SPECS[command];
  return factory ? factory(request) : null;
}

export const COMMANDS = Object.keys(SPECS);
