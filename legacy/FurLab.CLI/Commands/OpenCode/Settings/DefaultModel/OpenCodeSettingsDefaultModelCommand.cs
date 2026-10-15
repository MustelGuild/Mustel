using System.ComponentModel;
using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Nodes;

using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.OpenCode.Settings.DefaultModel;

/// <summary>
/// Sets the default model in the OpenCode configuration file.
/// </summary>
public sealed class OpenCodeSettingsDefaultModelCommand : AsyncCommand<OpenCodeSettingsDefaultModelSettings>
{
    private static readonly string GlobalConfigDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        ".config", "opencode");

    /// <summary>
    /// Hook that overrides model fetching in tests. When non-null, replaces the
    /// real <c>opencode models</c> process call inside <see cref="GetAvailableModels"/>.
    /// </summary>
    internal static Func<IReadOnlyList<string>>? ModelsProvider { get; set; }

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, OpenCodeSettingsDefaultModelSettings settings, CancellationToken cancellation)
    {
        string selectedModel;

        if (settings.ModelId is not null)
        {
            try
            {
                var availableModels = GetAvailableModels();
                if (!availableModels.Contains(settings.ModelId, StringComparer.Ordinal))
                {
                    Console.Error.WriteLine($"Model '{settings.ModelId}' not found.");
                    Console.Error.WriteLine("Available models:");
                    foreach (var m in availableModels)
                    {
                        Console.Error.WriteLine($"  {m}");
                    }

                    return Task.FromResult(1);
                }
            }
            catch (InvalidOperationException)
            {
                AnsiConsole.MarkupLine(
                    "[yellow]Warning:[/] Could not validate model (opencode not found in PATH). Setting without validation.");
            }

            selectedModel = settings.ModelId;
        }
        else
        {
            IReadOnlyList<string> availableModels;
            try
            {
                availableModels = GetAvailableModels();
            }
            catch (InvalidOperationException ex)
            {
                AnsiConsole.MarkupLine($"[red]Error:[/] {ex.Message}");
                AnsiConsole.MarkupLine(
                    "[yellow]Tip:[/] Provide the model ID directly: FurLab opencode settings default-model [grey]<model-id>[/]");
                return Task.FromResult(1);
            }

            var chosen = SelectModelInteractively(availableModels);
            if (chosen is null)
            {
                return Task.FromResult(0);
            }

            selectedModel = chosen;
        }

        var configPath = ResolveConfigPath(settings.Global);
        var configDir = Path.GetDirectoryName(configPath)
            ?? throw new InvalidOperationException($"Could not determine configuration file directory: {configPath}");
        Directory.CreateDirectory(configDir);

        var config = LoadConfigFile(configPath);
        config["model"] = selectedModel;
        SaveConfigFile(configPath, config);

        AnsiConsole.MarkupLine($"[green]Model '{selectedModel}' set at: {configPath}[/]");

        return Task.FromResult(0);
    }

    /// <summary>
    /// Resolves the config file path to use based on scope and existing files.
    /// Priority: opencode.jsonc > opencode.json > creates opencode.jsonc
    /// </summary>
    /// <param name="global">If true, resolves within the global OpenCode config directory.</param>
    /// <returns>The resolved file path.</returns>
    public static string ResolveConfigPath(bool global)
    {
        var directory = global ? GlobalConfigDir : Directory.GetCurrentDirectory();

        var jsonc = Path.Combine(directory, "opencode.jsonc");
        var json = Path.Combine(directory, "opencode.json");

        if (File.Exists(jsonc)) return jsonc;
        if (File.Exists(json)) return json;
        return jsonc;
    }

    /// <summary>
    /// Looks for the opencode executable in known installation locations.
    /// Falls back to a plain <c>"opencode"</c> name so the OS PATH is tried last.
    /// </summary>
    /// <returns>Full path to the executable (or script), or <c>"opencode"</c> if no known path was found.</returns>
    internal static string ResolveOpenCodeExecutable()
    {
        var localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);

        var wingetPackages = Path.Combine(localAppData, "Microsoft", "WinGet", "Packages");
        if (Directory.Exists(wingetPackages))
        {
            foreach (var dir in Directory.GetDirectories(wingetPackages, "SST.opencode_*"))
            {
                var exe = Path.Combine(dir, "opencode.exe");
                if (File.Exists(exe)) return exe;
            }
        }

        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        var npmPs1 = Path.Combine(appData, "npm", "opencode.ps1");
        if (File.Exists(npmPs1)) return npmPs1;

        return "opencode";
    }

    /// <summary>
    /// Returns the list of available models from the <c>opencode models</c> command.
    /// </summary>
    /// <returns>A read-only list of model IDs.</returns>
    /// <exception cref="InvalidOperationException">Thrown when opencode is not found.</exception>
    public static IReadOnlyList<string> GetAvailableModels()
    {
        if (ModelsProvider is not null)
        {
            return ModelsProvider();
        }

        var executable = ResolveOpenCodeExecutable();

        try
        {
            using var process = new Process();
            if (executable.EndsWith(".ps1", StringComparison.OrdinalIgnoreCase))
            {
                process.StartInfo = new ProcessStartInfo
                {
                    FileName = "pwsh.exe",
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    UseShellExecute = false,
                    CreateNoWindow = true
                };
                process.StartInfo.ArgumentList.Add("-NonInteractive");
                process.StartInfo.ArgumentList.Add("-NoProfile");
                process.StartInfo.ArgumentList.Add("-File");
                process.StartInfo.ArgumentList.Add(executable);
                process.StartInfo.ArgumentList.Add("models");
            }
            else
            {
                process.StartInfo = new ProcessStartInfo
                {
                    FileName = executable,
                    RedirectStandardOutput = true,
                    RedirectStandardError = true,
                    UseShellExecute = false,
                    CreateNoWindow = true
                };
                process.StartInfo.ArgumentList.Add("models");
            }

            process.Start();
            var output = process.StandardOutput.ReadToEnd();
            process.WaitForExit();

            return output
                .Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries)
                .Select(l => l.Trim())
                .Where(l => l.Length > 0)
                .ToList()
                .AsReadOnly();
        }
        catch (Win32Exception ex)
        {
            throw new InvalidOperationException(
                """
                Could not find the OpenCode executable.
                Try one of the options below:
                  1. Install the CLI via WinGet:  winget install SST.opencode
                  2. Add the installation directory to PATH manually.
                  3. Provide the model-id directly:  FurLab opencode settings default-model <model-id>
                """, ex);
        }
    }

    /// <summary>
    /// Displays an interactive selection prompt for the user to choose a model.
    /// </summary>
    /// <param name="models">The list of available models.</param>
    /// <returns>The selected model ID, or null if the user cancelled.</returns>
    public static string? SelectModelInteractively(IReadOnlyList<string> models)
    {
        try
        {
            return AnsiConsole.Prompt(
                new SelectionPrompt<string>()
                    .Title("Select the default model:")
                    .PageSize(15)
                    .MoreChoicesText("[grey](Move up and down to see more models)[/]")
                    .AddChoices(models));
        }
        catch (OperationCanceledException)
        {
            return null;
        }
    }

    private static JsonObject LoadConfigFile(string path)
    {
        if (!File.Exists(path))
        {
            return new JsonObject();
        }

        var json = File.ReadAllText(path);
        if (string.IsNullOrWhiteSpace(json))
        {
            return new JsonObject();
        }

        var node = JsonNode.Parse(json, null, new JsonDocumentOptions
        {
            CommentHandling = JsonCommentHandling.Skip
        });

        if (node is JsonObject objectNode)
        {
            return objectNode;
        }

        throw new InvalidDataException($"The file '{path}' does not contain a valid JSON object.");
    }

    private static void SaveConfigFile(string path, JsonObject config)
    {
        var json = config.ToJsonString(new JsonSerializerOptions { WriteIndented = true });
        File.WriteAllText(path, json + Environment.NewLine);
    }
}
