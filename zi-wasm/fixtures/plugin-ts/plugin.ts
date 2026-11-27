import { addLanguage, addLanguageServer, getMode, setMode } from "zi:api/editor";
import type * as Plugin from "zi:api/plugin";

export const dependency: typeof Plugin.dependency = {
  getName: () => "test-ts",
  dependencies: () => [],
};

export const lifecycle: typeof Plugin.lifecycle = {
  initialize() {
    try {
      addLanguage("typescript", ["test-ls"]);
      throw new Error("expected `addLanguage` with an undefined server to fail");
    } catch (err) {
      const payload = (err as { payload?: unknown }).payload;
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
  exec(cmd: string, _args: string[], _force: boolean): void {
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

export const command: typeof Plugin.command = { Handler };
