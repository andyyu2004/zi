import { setMode } from "zi:api/editor";

/** @type {typeof import("zi:api/plugin").dependency} */
export const dependency = {
  getName: () => "zi-plugin-template",
  dependencies: () => [],
};

/** @type {typeof import("zi:api/plugin").lifecycle} */
export const lifecycle = {
  initialize() {
    return { commands: [{ name: "hello", arity: { min: 0, max: 0 }, opts: {} }] };
  },
  shutdown() {},
};

class Handler {
  /**
   * @param {string} cmd
   * @param {string[]} _args
   * @param {boolean} _force
   */
  exec(cmd, _args, _force) {
    switch (cmd) {
      case "hello":
        setMode({ tag: "insert" });
        return;
      default:
        throw { tag: "message", val: `unknown command: \`${cmd}\`` };
    }
  }
}

/** @type {typeof import("zi:api/plugin").command} */
export const command = { Handler };
