namespace FurLab.Core.Models;

/// <summary>
/// Represents a single PostgreSQL server configuration entry.
/// </summary>
public class ServerConfigEntry
{
    /// <summary>
    /// Unique name/identifier for this server.
    /// </summary>
    public string Name { get; set; } = string.Empty;

    /// <summary>
    /// Database host address.
    /// </summary>
    public string Host { get; set; } = string.Empty;

    /// <summary>
    /// Database port.
    /// </summary>
    public int Port { get; set; } = 5432;

    /// <summary>
    /// Database username.
    /// </summary>
    public string Username { get; set; } = string.Empty;

    /// <summary>
    /// Encrypted password blob (base64). Managed by <c>ICredentialService</c>.
    /// Use <c>ICredentialService.TryDecrypt</c> to obtain the plaintext at runtime.
    /// </summary>
    public string? EncryptedPassword { get; set; }

    /// <summary>
    /// Specific databases to query. If empty and FetchAllDatabases is false, uses default database.
    /// </summary>
    public List<string> Databases { get; set; } = [];

    /// <summary>
    /// When true, automatically discovers all databases on the server.
    /// </summary>
    public bool FetchAllDatabases { get; set; }

    /// <summary>
    /// Patterns to exclude when auto-discovering databases (e.g., "template*", "postgres").
    /// </summary>
    public List<string> ExcludePatterns { get; set; } = ["template*", "postgres"];

    /// <summary>
    /// SSL mode (Disable, Allow, Prefer, Require, VerifyCA, VerifyFull).
    /// </summary>
    public string SslMode { get; set; } = "Prefer";

    /// <summary>
    /// Connection timeout in seconds.
    /// </summary>
    public int Timeout { get; set; } = 30;

    /// <summary>
    /// Maximum degree of parallelism for query execution on this server.
    /// </summary>
    public int MaxParallelism { get; set; } = 4;
}
