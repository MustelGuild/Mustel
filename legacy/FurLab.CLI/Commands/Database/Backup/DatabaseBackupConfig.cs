namespace FurLab.CLI.Commands.Database.Backup;

/// <summary>
/// Represents the configuration for a database backup operation.
/// </summary>
public sealed class DatabaseBackupConfig
{
    /// <summary>
    /// Gets or sets the database host address.
    /// </summary>
    public string Host { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets the database port.
    /// </summary>
    public string Port { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets the database username.
    /// </summary>
    public string? Username { get; set; }

    /// <summary>
    /// Gets or sets the database password.
    /// </summary>
    public string? Password { get; set; }

    /// <summary>
    /// Gets or sets the name of the database to backup.
    /// </summary>
    public string DatabaseName { get; set; } = string.Empty;

    /// <summary>
    /// Gets or sets a value indicating whether to backup all databases on the server.
    /// </summary>
    public bool BackupAll { get; set; }

    /// <summary>
    /// Gets or sets the table patterns whose data should be excluded from the backup.
    /// </summary>
    public string[]? ExcludeTableData { get; set; }

    /// <summary>
    /// Gets or sets the output file path (single database) or directory path (--all).
    /// </summary>
    public string? OutputPath { get; set; }
}
