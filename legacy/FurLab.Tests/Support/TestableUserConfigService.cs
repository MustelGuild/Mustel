using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using FurLab.Core.Interfaces;
using FurLab.Core.Models;

namespace FurLab.Tests.Support;

/// <summary>
/// A testable version of UserConfigService that allows specifying the config folder.
/// </summary>
public sealed class TestableUserConfigService(Core.Logging.ILogger logger, string configFolder) : IUserConfigService
{
    private readonly Core.Logging.ILogger _logger = logger;
    private readonly string _configFilePath = Path.Combine(configFolder, "furlab.jsonc");
    private readonly string _configFolder = configFolder;

    public UserConfig LoadConfig()
    {
        if (!ConfigFileExists())
        {
            var config = new UserConfig { Defaults = new UserDefaults() };
            SaveConfig(config);
            return config;
        }

        var json = File.ReadAllText(_configFilePath);
        var strippedJson = StripJsonComments(json);
        var deserialized = JsonSerializer.Deserialize<UserConfig>(strippedJson, new JsonSerializerOptions
        {
            PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
            PropertyNameCaseInsensitive = true
        });

        if (deserialized == null)
        {
            var config = new UserConfig { Defaults = new UserDefaults() };
            SaveConfig(config);
            return config;
        }

        ApplyDefaults(deserialized);
        return deserialized;
    }

    public void SaveConfig(UserConfig config)
    {
        ArgumentNullException.ThrowIfNull(config);
        Directory.CreateDirectory(_configFolder);
        var json = JsonSerializer.Serialize(config, new JsonSerializerOptions { WriteIndented = true, PropertyNamingPolicy = JsonNamingPolicy.CamelCase });
        File.WriteAllText(_configFilePath, json);
    }

    public IReadOnlyList<ServerConfigEntry> GetServers() => LoadConfig().Servers.AsReadOnly();

    public ServerConfigEntry? GetServer(string name) => LoadConfig().Servers.FirstOrDefault(s => s.Name.Equals(name, StringComparison.OrdinalIgnoreCase));

    public void AddOrUpdateServer(ServerConfigEntry server)
    {
        var config = LoadConfig();
        var idx = config.Servers.FindIndex(s => s.Name.Equals(server.Name, StringComparison.OrdinalIgnoreCase));
        if (idx >= 0) config.Servers[idx] = server;
        else config.Servers.Add(server);
        SaveConfig(config);
    }

    public bool RemoveServer(string name)
    {
        var config = LoadConfig();
        var server = config.Servers.FirstOrDefault(s => s.Name.Equals(name, StringComparison.OrdinalIgnoreCase));
        if (server == null) return false;
        config.Servers.Remove(server);
        SaveConfig(config);
        return true;
    }

    public UserDefaults GetDefaults() => LoadConfig().Defaults ?? new UserDefaults();

    public string GetConfigFilePath() => _configFilePath;

    public bool ConfigFileExists() => File.Exists(_configFilePath);

    public void SetEncryptedPassword(string serverName, string encryptedPassword)
    {
        var config = LoadConfig();
        var server = config.Servers.FirstOrDefault(s => s.Name.Equals(serverName, StringComparison.OrdinalIgnoreCase));
        if (server == null) throw new ArgumentException($"Server '{serverName}' not found.", nameof(serverName));
        server.EncryptedPassword = encryptedPassword;
        SaveConfig(config);
    }

    private static void ApplyDefaults(UserConfig config)
    {
        config.Defaults ??= new UserDefaults();
        foreach (var server in config.Servers)
        {
            if (server.Port == 0) server.Port = 5432;
            if (string.IsNullOrWhiteSpace(server.SslMode)) server.SslMode = "Prefer";
            if (server.Timeout == 0) server.Timeout = 30;
            if (server.MaxParallelism == 0) server.MaxParallelism = 4;
            if (server.ExcludePatterns == null || server.ExcludePatterns.Count == 0)
                server.ExcludePatterns = ["template*", "postgres"];
        }
    }

    private static string StripJsonComments(string jsonc)
    {
        return System.Text.RegularExpressions.Regex.Replace(jsonc, @"//.*?$|/\*[\s\S]*?\*/", "", System.Text.RegularExpressions.RegexOptions.Multiline);
    }
}
