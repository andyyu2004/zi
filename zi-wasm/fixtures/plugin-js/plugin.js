import { addLanguage, addLanguageServer, getMode, setMode } from "zi:api/editor";

/** @type {typeof import("zi:api/plugin").dependency} */
export const dependency = {
  getName: () => "test-js",
  dependencies: () => [],
};

/** @type {typeof import("zi:api/plugin").lifecycle} */
export const lifecycle = {
  initialize() {
    try {
      addLanguage("typescript", ["test-ls"]);
      throw new Error("expected `addLanguage` with an undefined server to fail");
    } catch (err) {
      const payload = /** @type {{ payload?: unknown }} */ (err).payload;
      if (payload !== "language server `test-ls` for language `typescript` is not defined") {
        throw err;
      }
    }

    addLanguageServer("test-ls", { command: "test-ls", args: ["--stdio"] });
    addLanguage("typescript", ["test-ls"]);

    return { commands: [{ name: "insert-mode", arity: { min: 0, max: 0 }, opts: {} }] };
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
      case "insert-mode":
        setMode({ tag: "insert" });
        if (getMode().tag !== "insert") {
          throw new Error("expected insert mode");
        }
        return;
      default:
        throw { tag: "message", val: `unexpected command: \`${cmd}\`` };
    }
  }
}

/** @type {typeof import("zi:api/plugin").command} */
export const command = { Handler };
