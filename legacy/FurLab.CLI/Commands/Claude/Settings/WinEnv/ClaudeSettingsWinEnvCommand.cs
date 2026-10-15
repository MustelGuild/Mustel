using System.Text.Json;
using System.Text.Json.Nodes;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Claude.Settings.WinEnv;

/// <summary>
/// Configures the Windows environment for Claude Code by updating ~/.claude.json permissions
/// and creating a CLAUDE.md file with global shell rules.
/// </summary>
public sealed class ClaudeSettingsWinEnvCommand : AsyncCommand<ClaudeSettingsWinEnvSettings>
{
    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, ClaudeSettingsWinEnvSettings settings, CancellationToken cancellation)
    {
        if (!OperatingSystem.IsWindows())
        {
            throw new PlatformNotSupportedException("The 'claude settings win-env' command is only supported on Windows.");
        }

        var configPath = GetUserClaudeConfigPath();
        var config = LoadSettingsFile(configPath);

        config["permission"] = new JsonObject
        {
            ["edit"] = "allow",
            ["read"] = "allow",
            ["shell"] = "allow"
        };

        SaveSettingsFile(configPath, config);
        Console.WriteLine($"Configuration file updated: {configPath}");

        var claudeMdPath = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".claude", "CLAUDE.md");
        var claudeMdDirectory = Path.GetDirectoryName(claudeMdPath);

        if (claudeMdDirectory != null && !Directory.Exists(claudeMdDirectory))
        {
            Directory.CreateDirectory(claudeMdDirectory);
        }

        var claudeMdContent = @"# Global Shell Rules (Windows + Git Bash)
When executing commands in Git Bash:
- NEVER use: > nul, 2>nul, >NUL, 2>NUL
- ALWAYS use: >/dev/null 2>&1 to suppress output
- ALWAYS use: 2>/dev/null to suppress stderr only
Environment:
- OS: Windows
- Shell for command execution: Git Bash (bash)
- Null device in this shell is: /dev/null
- ""nul"" is a Windows device name and must never be used as a filename.";

        File.WriteAllText(claudeMdPath, claudeMdContent);
        Console.WriteLine($"CLAUDE.md file created: {claudeMdPath}");

        return Task.FromResult(0);
    }

    private static string GetUserClaudeConfigPath()
    {
        return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".claude.json");
    }

    private static JsonObject LoadSettingsFile(string settingsPath)
    {
        if (!File.Exists(settingsPath))
        {
            return new JsonObject();
        }

        var json = File.ReadAllText(settingsPath);
        if (string.IsNullOrWhiteSpace(json))
        {
            return new JsonObject();
        }

        var node = JsonNode.Parse(json);
        if (node is JsonObject objectNode)
        {
            return objectNode;
        }

        throw new InvalidDataException($"The file '{settingsPath}' does not contain a valid JSON object.");
    }

    private static void SaveSettingsFile(string settingsPath, JsonObject settings)
    {
        var json = settings.ToJsonString(new JsonSerializerOptions
        {
            WriteIndented = true
        });

        File.WriteAllText(settingsPath, json + Environment.NewLine);
    }
}
