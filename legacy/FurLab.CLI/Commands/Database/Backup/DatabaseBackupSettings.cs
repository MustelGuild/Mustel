using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Database.Backup;

/// <summary>
/// Settings for the database backup command.
/// </summary>
public sealed class DatabaseBackupSettings : CommandSettings
{
    /// <summary>
    /// Gets the name of the database to backup.
    /// Not required when using <see cref="All"/>.
    /// </summary>
    [CommandArgument(0, "[database]")]
    [Description("Name of the database to backup. Not required when using --all.")]
    public string? Database { get; init; }

    /// <summary>
    /// Gets the database host address.
    /// </summary>
    [CommandOption("-H|--host")]
    [Description("Database host address.")]
    public string? Host { get; init; }

    /// <summary>
    /// Gets the database port.
    /// </summary>
    [CommandOption("-p|--port")]
    [Description("Database port.")]
    public string? Port { get; init; }

    /// <summary>
    /// Gets the database username.
    /// </summary>
    [CommandOption("-U|--username")]
    [Description("Database username.")]
    public string? Username { get; init; }

    /// <summary>
    /// Gets the database password.
    /// If not provided, will be resolved from user config or prompted interactively.
    /// </summary>
    [CommandOption("-W|--password")]
    [Description("Database password. If not provided, will be resolved from user config or prompted interactively.")]
    public string? Password { get; init; }

    /// <summary>
    /// Gets the SSL mode for the connection.
    /// </summary>
    [CommandOption("--ssl-mode")]
    [Description("SSL mode. Default: Prefer.")]
    public string? SslMode { get; init; }

    /// <summary>
    /// Gets the connection timeout in seconds.
    /// </summary>
    [CommandOption("--timeout")]
    [Description("Connection timeout in seconds. Default: 30.")]
    public int? Timeout { get; init; }

    /// <summary>
    /// Gets the command timeout in seconds.
    /// </summary>
    [CommandOption("--command-timeout")]
    [Description("Command timeout in seconds. Default: 300.")]
    public int? CommandTimeout { get; init; }

    /// <summary>
    /// Gets a value indicating whether to backup all databases on the server.
    /// </summary>
    [CommandOption("-a|--all")]
    [Description("Backup all databases on the server. Each database will have its own .dump file.")]
    public bool All { get; init; }

    /// <summary>
    /// Gets the table patterns to exclude entirely from the backup.
    /// </summary>
    [CommandOption("--exclude")]
    [Description("Exclude tables matching the specified pattern(s). Can be specified multiple times. Example: --exclude 'log*'")]
    public string[]? Exclude { get; init; }

    /// <summary>
    /// Gets the table patterns whose data should be excluded from the backup.
    /// </summary>
    [CommandOption("--exclude-table-data")]
    [Description("Exclude table data matching the specified pattern(s). Can be specified multiple times. Example: --exclude-table-data 'log*'")]
    public string[]? ExcludeTableData { get; init; }

    /// <summary>
    /// Gets a value indicating whether to enable verbose output from pg_dump.
    /// </summary>
    [CommandOption("-v|--verbose")]
    [Description("Enable verbose output from pg_dump.")]
    public bool Verbose { get; init; }

    /// <summary>
    /// Gets the output file path or directory path.
    /// </summary>
    [CommandOption("-o|--output")]
    [Description("Output file path (for single database) or folder path (for --all). If not provided, uses current directory.")]
    public string? Output { get; init; }

    /// <summary>
    /// Gets the output format for the backup.
    /// </summary>
    [CommandOption("-F|--format")]
    [Description("Output format: p (plain), c (custom), d (directory), t (tar). Default: c.")]
    public string? Format { get; init; }
}
