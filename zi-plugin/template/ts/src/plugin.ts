import { setMode } from "zi:api/editor";
import type * as Plugin from "zi:api/plugin";

export const dependency: typeof Plugin.dependency = {
  getName: () => "zi-plugin-template",
  dependencies: () => [],
};

export const lifecycle: typeof Plugin.lifecycle = {
  initialize() {
    return { commands: [{ name: "hello", arity: { min: 0, max: 0 }, opts: {} }] };
  },
  shutdown() {},
};

class Handler {
  exec(cmd: string, _args: string[], _force: boolean): void {
    switch (cmd) {
      case "hello":
        setMode({ tag: "insert" });
        return;
      default:
        throw { tag: "message", val: `unknown command: \`${cmd}\`` };
    }
  }
}

export const command: typeof Plugin.command = { Handler };
