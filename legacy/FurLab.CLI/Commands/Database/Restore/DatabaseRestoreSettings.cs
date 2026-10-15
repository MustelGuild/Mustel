using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Database.Restore;

/// <summary>
/// Settings for the database restore command.
/// </summary>
public sealed class DatabaseRestoreSettings : CommandSettings
{
    /// <summary>
    /// Gets the name of the database to restore.
    /// Not required when using <see cref="All"/>.
    /// </summary>
    [CommandArgument(0, "[database]")]
    [Description("Name of the database to restore. Not required when using --all.")]
    public string? Database { get; init; }

    /// <summary>
    /// Gets the path to the dump file to restore.
    /// If not provided, looks for <c>&lt;database&gt;.dump</c> in the current directory.
    /// </summary>
    [CommandArgument(1, "[file]")]
    [Description("Path to the dump file to restore. If not provided, looks for <database>.dump in the current directory.")]
    public string? InputFile { get; init; }

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
    /// Gets the SSL mode for the database connection.
    /// </summary>
    [CommandOption("--ssl-mode")]
    [Description("SSL mode for the database connection (Disable, Prefer, Require, VerifyCA, VerifyFull).")]
    public string? SslMode { get; init; }

    /// <summary>
    /// Gets the timeout in seconds for the restore operation.
    /// </summary>
    [CommandOption("--timeout")]
    [Description("Timeout in seconds for the restore operation.")]
    public int? Timeout { get; init; }

    /// <summary>
    /// Gets the command timeout in seconds for the restore operation.
    /// </summary>
    [CommandOption("--command-timeout")]
    [Description("Command timeout in seconds for the restore operation.")]
    public int? CommandTimeout { get; init; }

    /// <summary>
    /// Gets a value indicating whether to restore all databases from dump files.
    /// </summary>
    [CommandOption("-a|--all")]
    [Description("Restore all databases from dump files in the specified directory.")]
    public bool All { get; init; }

    /// <summary>
    /// Gets the directory containing .dump files for <see cref="All"/> mode.
    /// </summary>
    [CommandOption("-d|--directory")]
    [Description("Directory containing .dump files (for --all). If not provided, uses current directory.")]
    public string? Directory { get; init; }

    /// <summary>
    /// Gets a value indicating whether to enable verbose output from pg_restore.
    /// </summary>
    [CommandOption("-v|--verbose")]
    [Description("Enable verbose output from pg_restore.")]
    public bool Verbose { get; init; }

    /// <summary>
    /// Gets a value indicating whether to skip the clean (drop) step before restoring.
    /// By default, pg_restore runs with <c>-c</c> to drop objects before creating them.
    /// </summary>
    [CommandOption("--no-clean")]
    [Description("Do not drop database objects before creating them. By default, --clean is used.")]
    public bool NoClean { get; init; }

    /// <summary>
    /// Gets a value indicating whether to skip restoration of object ownership.
    /// </summary>
    [CommandOption("--no-owner")]
    [Description("Skip restoration of object ownership.")]
    public bool NoOwner { get; init; }

    /// <summary>
    /// Gets a value indicating whether to skip restoration of access privileges.
    /// </summary>
    [CommandOption("--no-acl")]
    [Description("Skip restoration of access privileges.")]
    public bool NoAcl { get; init; }

    /// <summary>
    /// Gets a value indicating whether to execute the restore as a single transaction.
    /// </summary>
    [CommandOption("--single-transaction")]
    [Description("Execute the restore as a single transaction.")]
    public bool SingleTransaction { get; init; }

    /// <summary>
    /// Gets the number of parallel jobs for restore (custom-format archives only).
    /// </summary>
    [CommandOption("-j|--jobs")]
    [Description("Number of parallel jobs for restore (custom-format only).")]
    public int? Jobs { get; init; }
}
