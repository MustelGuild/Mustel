using FurLab.Core.Models;

namespace FurLab.Core.Interfaces;

/// <summary>
/// Defines a service for managing user-level configuration (furlab.jsonc).
/// </summary>
public interface IUserConfigService
{
    /// <summary>
    /// Loads the complete user configuration from furlab.jsonc.
    /// Creates the file with defaults if it doesn't exist.
    /// </summary>
    UserConfig LoadConfig();

    /// <summary>
    /// Saves the complete user configuration to furlab.jsonc.
    /// </summary>
    void SaveConfig(UserConfig config);

    /// <summary>
    /// Gets all configured servers.
    /// </summary>
    IReadOnlyList<ServerConfigEntry> GetServers();

    /// <summary>
    /// Gets a server by name.
    /// </summary>
    ServerConfigEntry? GetServer(string name);

    /// <summary>
    /// Adds or updates a server configuration.
    /// </summary>
    void AddOrUpdateServer(ServerConfigEntry server);

    /// <summary>
    /// Removes a server by name.
    /// </summary>
    bool RemoveServer(string name);

    /// <summary>
    /// Gets the default settings.
    /// </summary>
    UserDefaults GetDefaults();

    /// <summary>
    /// Gets the path to the user config file.
    /// </summary>
    string GetConfigFilePath();

    /// <summary>
    /// Checks if the user config file exists.
    /// </summary>
    bool ConfigFileExists();

    /// <summary>
    /// Sets the encrypted password for a server identified by name.
    /// </summary>
    /// <param name="serverName">The server name.</param>
    /// <param name="encryptedPassword">The encrypted password blob to store.</param>
    /// <exception cref="ArgumentException">Thrown when the server is not found.</exception>
    void SetEncryptedPassword(string serverName, string encryptedPassword);
}
