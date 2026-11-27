import { addLanguage, addLanguageServer, getMode, setMode } from "zi:api/editor";
import type * as Plugin from "zi:api/plugin";

export const dependency: typeof Plugin.dependency = {
  getName: () => "test-ts",
  dependencies: () => [],
};

export const lifecycle: typeof Plugin.lifecycle = {
  initialize() {
    try {
      addLanguage("typescript", ["test-ts-ls"]);
      throw new Error("expected `addLanguage` with an undefined server to fail");
    } catch (err) {
      const payload = (err as { payload?: unknown }).payload;
      if (payload !== "language server `test-ts-ls` for language `typescript` is not defined") {
        throw err;
      }
    }

    addLanguageServer("test-ts-ls", { command: "test-ts-ls", args: ["--stdio"] });
    addLanguage("typescript", ["test-ts-ls"]);

    return { commands: [{ name: "ts-insert", arity: { min: 0, max: 0 }, opts: {} }] };
  },
  shutdown() {},
};

class Handler {
  exec(cmd: string, _args: string[], _force: boolean): void {
    switch (cmd) {
      case "ts-insert":
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
