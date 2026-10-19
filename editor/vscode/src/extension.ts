// VS Code client for `hvolt lsp`.
//
// The extension stays thin: the language server does the work, and the commands it
// exposes are thin wrappers that call the server's `workspace/executeCommand`, so the
// editor and any other LSP client share one implementation.
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions, type ServerOptions } from "vscode-languageclient/node";

const COMMANDS = ["hvolt.build", "hvolt.run", "hvolt.sign", "hvolt.push", "hvolt.screenshot", "hvolt.log"];

interface CommandResult {
  ok: boolean;
  seconds?: string;
  tail?: string;
  message?: string;
}

let client: LanguageClient | undefined;
let status: vscode.StatusBarItem;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const config = vscode.workspace.getConfiguration("hvolt");
  const binary = config.get<string>("binaryPath") || "hvolt";
  const sdk = config.get<string>("sdkPath") || "";
  const root = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;

  status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left);
  status.text = "hvolt";
  status.tooltip = "hvolt language server";
  status.show();
  context.subscriptions.push(status);

  if (root) {
    const args = ["lsp", "--stdio"];
    if (sdk) args.push("--sdk", sdk);
    const server: ServerOptions = { command: binary, args, options: { cwd: root } };
    const clientOptions: LanguageClientOptions = {
      documentSelector: [{ scheme: "file", language: "ets" }, { scheme: "file", pattern: "**/hvolt.toml" }],
      workspaceFolder: vscode.workspace.workspaceFolders?.[0],
      initializationOptions: {
        hvolt: {
          binaryPath: binary,
          lintOnSave: config.get<boolean>("lintOnSave") ?? true,
        },
      },
      synchronize: { fileEvents: vscode.workspace.createFileSystemWatcher("**/hvolt.toml") },
    };

    client = new LanguageClient("hvolt", "hvolt", server, clientOptions);
    client.start().catch((error: Error) => {
      status.text = "hvolt: failed";
      status.tooltip = String(error);
      void vscode.window.showErrorMessage(`hvolt: language server failed to start — ${error.message}`);
    });
  } else {
    void vscode.window.showWarningMessage("hvolt: open an hvolt project folder first (hvolt.toml expected)");
  }

  for (const command of COMMANDS) {
    context.subscriptions.push(
      vscode.commands.registerCommand(command, async () => {
        if (!client || !root) {
          void vscode.window.showErrorMessage("hvolt: no workspace with hvolt.toml");
          return;
        }
        const options = {
          device: config.get<string>("device") || undefined,
        };
        status.text = `hvolt: ${command.replace("hvolt.", "")}…`;
        try {
          const result = (await client.sendRequest("workspace/executeCommand", {
            command,
            arguments: [root, options],
          })) as CommandResult | null;
          if (!result) {
            status.text = "hvolt";
            return;
          }
          const label = `${command.replace("hvolt.", "")}: ${result.ok ? "ok" : "failed"}${
            result.seconds ? ` (${result.seconds}s)` : ""
          }`;
          status.text = `hvolt: ${result.ok ? "ok" : "failed"}`;
          status.tooltip = result.tail ?? label;
          if (!result.ok) {
            const choice = await vscode.window.showErrorMessage(label, "Show output");
            if (choice === "Show output" && result.tail) {
              const channel = vscode.window.createOutputChannel("hvolt");
              channel.appendLine(result.tail);
              channel.show();
            }
          }
        } catch (error) {
          status.text = "hvolt: failed";
          void vscode.window.showErrorMessage(`hvolt: ${String(error)}`);
        }
      }),
    );
  }
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
