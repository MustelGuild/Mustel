using FurLab.Core.Constants;

namespace FurLab.Core.Models;

/// <summary>
/// Represents options for a database connection.
/// </summary>
public record DatabaseConnectionOptions
{
    /// <summary>
    /// Gets or sets the database host address.
    /// </summary>
    public string Host { get; init; } = FurLabConstants.DefaultHost;

    /// <summary>
    /// Gets or sets the database port.
    /// </summary>
    public string Port { get; init; } = FurLabConstants.DefaultPort;

    /// <summary>
    /// Gets or sets the database username.
    /// </summary>
    public string? Username { get; init; }

    /// <summary>
    /// Gets or sets the database password.
    /// </summary>
    public string? Password { get; init; }

    /// <summary>
    /// Gets or sets the SSL mode for the connection.
    /// </summary>
    public string? SslMode { get; init; }
}
