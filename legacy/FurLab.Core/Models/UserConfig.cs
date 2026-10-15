namespace FurLab.Core.Models;

/// <summary>
/// Represents the complete user configuration stored in furlab.jsonc.
/// </summary>
public class UserConfig
{
    /// <summary>
    /// List of configured PostgreSQL servers.
    /// </summary>
    public List<ServerConfigEntry> Servers { get; set; } = [];

    /// <summary>
    /// Default settings for query execution and output.
    /// </summary>
    public UserDefaults? Defaults { get; set; }
}
