using System.Text.Json;
using System.Text.RegularExpressions;

using FurLab.Core.Interfaces;
using FurLab.Core.Logging;
using FurLab.Core.Models;

namespace FurLab.Core.Services;

/// <summary>
/// Provides centralized user configuration management using furlab.jsonc.
/// </summary>
public partial class UserConfigService : IUserConfigService
{
    private readonly ILogger _logger;
    private readonly string _configFolder;
    private readonly string _configFilePath;

    /// <summary>
    /// Initializes a new instance of the <see cref="UserConfigService"/> class.
    /// </summary>
    /// <param name="logger">The logger instance.</param>
    public UserConfigService(ILogger logger)
    {
        _logger = logger ?? throw new ArgumentNullException(nameof(logger));
        _configFolder = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "FurLab");
        _configFilePath = Path.Combine(_configFolder, "furlab.jsonc");
    }

    /// <inheritdoc/>
    public UserConfig LoadConfig()
    {
        if (!ConfigFileExists())
        {
            var config = CreateDefaultConfig();
            SaveConfig(config);
            return config;
        }

        try
        {
            var json = File.ReadAllText(_configFilePath);
            var strippedJson = StripJsonComments(json);
            var config = JsonSerializer.Deserialize<UserConfig>(strippedJson, GetSerializerOptions());

            if (config == null)
            {
                _logger.LogWarning("furlab.jsonc deserialized to null. Creating default config.");
                config = CreateDefaultConfig();
                SaveConfig(config);
            }
            else
            {
                ValidateAndApplyDefaults(config);
            }

            return config;
        }
        catch (JsonException ex)
        {
            _logger.LogError($"Failed to parse furlab.jsonc: {ex.Message}");
            throw new InvalidOperationException($"Invalid JSON in furlab.jsonc: {ex.Message}", ex);
        }
        catch (Exception ex) when (ex is not InvalidOperationException)
        {
            _logger.LogError($"Failed to load furlab.jsonc: {ex.Message}");
            throw new InvalidOperationException($"Failed to load configuration: {ex.Message}", ex);
        }
    }

    /// <inheritdoc/>
    public void SaveConfig(UserConfig config)
    {
        ArgumentNullException.ThrowIfNull(config);

        Directory.CreateDirectory(_configFolder);

        var options = new JsonSerializerOptions
        {
            WriteIndented = true,
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase
        };

        var json = JsonSerializer.Serialize(config, options);
        File.WriteAllText(_configFilePath, json);

        _logger.LogDebug($"Configuration saved to {_configFilePath}");
    }

    /// <inheritdoc/>
    public IReadOnlyList<ServerConfigEntry> GetServers()
    {
        var config = LoadConfig();
        return config.Servers.AsReadOnly();
    }

    /// <inheritdoc/>
    public ServerConfigEntry? GetServer(string name)
    {
        var config = LoadConfig();
        return config.Servers.FirstOrDefault(s => s.Name.Equals(name, StringComparison.OrdinalIgnoreCase));
    }

    /// <inheritdoc/>
    public void AddOrUpdateServer(ServerConfigEntry server)
    {
        ArgumentNullException.ThrowIfNull(server);

        if (string.IsNullOrWhiteSpace(server.Name))
        {
            throw new ArgumentException("Server name cannot be empty.", nameof(server));
        }

        var config = LoadConfig();
        var existingIndex = config.Servers.FindIndex(s => s.Name.Equals(server.Name, StringComparison.OrdinalIgnoreCase));

        if (existingIndex >= 0)
        {
            config.Servers[existingIndex] = server;
            _logger.LogDebug($"Server '{server.Name}' updated.");
        }
        else
        {
            config.Servers.Add(server);
            _logger.LogDebug($"Server '{server.Name}' added.");
        }

        SaveConfig(config);
    }

    /// <inheritdoc/>
    public bool RemoveServer(string name)
    {
        var config = LoadConfig();
        var server = config.Servers.FirstOrDefault(s => s.Name.Equals(name, StringComparison.OrdinalIgnoreCase));

        if (server == null)
        {
            return false;
        }

        config.Servers.Remove(server);
        SaveConfig(config);
        _logger.LogDebug($"Server '{name}' removed.");
        return true;
    }

    /// <inheritdoc/>
    public UserDefaults GetDefaults()
    {
        var config = LoadConfig();
        return config.Defaults ?? new UserDefaults();
    }

    /// <inheritdoc/>
    public string GetConfigFilePath() => _configFilePath;

    /// <inheritdoc/>
    public bool ConfigFileExists() => File.Exists(_configFilePath);

    /// <inheritdoc/>
    public void SetEncryptedPassword(string serverName, string encryptedPassword)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(serverName);
        ArgumentNullException.ThrowIfNull(encryptedPassword);

        var config = LoadConfig();
        var server = config.Servers.FirstOrDefault(s => s.Name.Equals(serverName, StringComparison.OrdinalIgnoreCase));

        if (server == null)
        {
            throw new ArgumentException($"Server '{serverName}' not found.", nameof(serverName));
        }

        server.EncryptedPassword = encryptedPassword;
        SaveConfig(config);
        _logger.LogDebug($"Encrypted password updated for server '{serverName}'.");
    }

    private static UserConfig CreateDefaultConfig()
    {
        return new UserConfig
        {
            Defaults = new UserDefaults()
        };
    }

    private static void ValidateAndApplyDefaults(UserConfig config)
    {
        config.Defaults ??= new UserDefaults();

        foreach (var server in config.Servers)
        {
            if (server.Port == 0) server.Port = 5432;
            if (string.IsNullOrWhiteSpace(server.SslMode)) server.SslMode = "Prefer";
            if (server.Timeout == 0) server.Timeout = 30;
            if (server.MaxParallelism == 0) server.MaxParallelism = 4;
            if (server.ExcludePatterns == null || server.ExcludePatterns.Count == 0)
            {
                server.ExcludePatterns = ["template*", "postgres"];
            }

        }
    }

    private static JsonSerializerOptions GetSerializerOptions()
    {
        return new JsonSerializerOptions
        {
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
            PropertyNameCaseInsensitive = true
        };
    }

    /// <summary>
    /// Strips single-line and multi-line comments from JSONC content.
    /// </summary>
    [GeneratedRegex(@"//.*?$|/\*[\s\S]*?\*/", RegexOptions.Multiline)]
    private static partial Regex JsonCommentRegex();

    private static string StripJsonComments(string jsonc)
    {
        return JsonCommentRegex().Replace(jsonc, string.Empty);
    }
}
