using FurLab.Core.Constants;

namespace FurLab.Core.Models;

/// <summary>
/// Represents a single line in the pgpass.conf file.
/// </summary>
public record PgPassEntry
{
    /// <summary>Hostname or IP address of the PostgreSQL server. Default: "localhost". Accepts "*" as a wildcard.</summary>
    public string Hostname { get; init; } = FurLabConstants.DefaultHost;

    /// <summary>TCP port of the PostgreSQL server. Default: "5432". Accepts "*" as a wildcard.</summary>
    public string Port { get; init; } = FurLabConstants.DefaultPort;

    /// <summary>Database name. Default: "*" (wildcard). Cannot be empty if provided explicitly.</summary>
    public string Database { get; init; } = "*";

    /// <summary>PostgreSQL username. Default: "postgres". Accepts "*" as a wildcard.</summary>
    public string Username { get; init; } = "postgres";

    /// <summary>PostgreSQL password. Never empty. Never a wildcard. Stored without escaping (escaping applied during serialization).</summary>
    public string Password { get; init; } = string.Empty;

    /// <summary>
    /// Identity key for duplicate detection.
    /// Two records with the same key are considered duplicates, regardless of the password.
    /// </summary>
    public (string Hostname, string Port, string Database, string Username) IdentityKey
        => (Hostname, Port, Database, Username);
}
