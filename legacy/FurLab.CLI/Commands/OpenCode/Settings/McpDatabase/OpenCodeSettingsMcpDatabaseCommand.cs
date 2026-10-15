using System.Text.Json;
using System.Text.Json.Nodes;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.OpenCode.Settings.McpDatabase;

/// <summary>
/// Configures the MCP database integration in the OpenCode config.json file.
/// </summary>
public sealed class OpenCodeSettingsMcpDatabaseCommand : AsyncCommand<OpenCodeSettingsMcpDatabaseSettings>
{
    private static readonly string GlobalConfigDir = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        ".config", "opencode");

    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, OpenCodeSettingsMcpDatabaseSettings settings, CancellationToken cancellation)
    {
        var configPath = Path.Combine(GlobalConfigDir, "config.json");

        var configDir = Path.GetDirectoryName(configPath)
            ?? throw new InvalidOperationException($"Could not determine configuration file directory: {configPath}");
        Directory.CreateDirectory(configDir);

        var config = LoadConfigFile(configPath);

        if (config["mcp"] is not JsonObject mcpNode)
        {
            mcpNode = new JsonObject();
            config["mcp"] = mcpNode;
        }

        mcpNode["toolbox"] = new JsonObject
        {
            ["type"] = "remote",
            ["url"] = "http://127.0.0.1:5000/mcp",
            ["enabled"] = true
        };

        SaveConfigFile(configPath, config);
        Console.WriteLine($"Configuration file updated: {configPath}");

        return Task.FromResult(0);
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
