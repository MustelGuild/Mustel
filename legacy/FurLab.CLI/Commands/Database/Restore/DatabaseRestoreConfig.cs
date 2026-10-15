using FurLab.Core.Constants;

namespace FurLab.CLI.Commands.Database.Restore;

/// <summary>
/// Represents the configuration for a database restore operation.
/// </summary>
internal record DatabaseRestoreConfig
{
    /// <summary>
    /// Gets the database host address.
    /// </summary>
    public string Host { get; init; } = FurLabConstants.DefaultHost;

    /// <summary>
    /// Gets the database port.
    /// </summary>
    public string Port { get; init; } = FurLabConstants.DefaultPort;

    /// <summary>
    /// Gets the database username.
    /// </summary>
    public string? Username { get; init; }

    /// <summary>
    /// Gets the database password.
    /// </summary>
    public string? Password { get; init; }

    /// <summary>
    /// Gets the name of the database to restore.
    /// </summary>
    public string DatabaseName { get; init; } = string.Empty;

    /// <summary>
    /// Gets a value indicating whether to restore all databases from dump files.
    /// </summary>
    public bool RestoreAll { get; init; }

    /// <summary>
    /// Gets the path to the input dump file.
    /// </summary>
    public string? InputFile { get; init; }

    /// <summary>
    /// Gets the directory containing dump files for restore-all mode.
    /// </summary>
    public string? OutputPath { get; init; }

    /// <summary>
    /// Gets the SSL mode for the connection.
    /// </summary>
    public string? SslMode { get; init; }

    /// <summary>
    /// Gets the timeout in seconds for the restore operation.
    /// </summary>
    public int? Timeout { get; init; }

    /// <summary>
    /// Gets the command timeout in seconds for the restore operation.
    /// </summary>
    public int? CommandTimeout { get; init; }
}
