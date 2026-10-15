using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.Add;

/// <summary>
/// Settings for the db-servers add command.
/// </summary>
public sealed class DbServersAddSettings : CommandSettings
{
    /// <summary>
    /// Gets the server name (unique identifier).
    /// </summary>
    [CommandOption("-n|--name")]
    [Description("Server name (unique identifier).")]
    public string? Name { get; init; }

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
    [Description("Database port. Default: 5432.")]
    public int Port { get; init; } = 5432;

    /// <summary>
    /// Gets the database username.
    /// </summary>
    [CommandOption("-U|--username")]
    [Description("Database username.")]
    public string? Username { get; init; }

    /// <summary>
    /// Gets the comma-separated list of specific databases.
    /// </summary>
    [CommandOption("-d|--databases")]
    [Description("Comma-separated list of specific databases.")]
    public string? Databases { get; init; }

    /// <summary>
    /// Gets the SSL mode.
    /// </summary>
    [CommandOption("--ssl-mode")]
    [Description("SSL mode (Disable, Allow, Prefer, Require, VerifyCA, VerifyFull).")]
    public string? SslMode { get; init; }

    /// <summary>
    /// Gets the connection timeout in seconds.
    /// </summary>
    [CommandOption("--timeout")]
    [Description("Connection timeout in seconds.")]
    public int? Timeout { get; init; }

    /// <summary>
    /// Gets the max degree of parallelism.
    /// </summary>
    [CommandOption("--max-parallelism")]
    [Description("Max degree of parallelism.")]
    public int? MaxParallelism { get; init; }

    /// <summary>
    /// Gets a value indicating whether to auto-discover all databases on server.
    /// </summary>
    [CommandOption("--fetch-all")]
    [Description("Auto-discover all databases on server.")]
    public bool FetchAllDatabases { get; init; }

    /// <summary>
    /// Gets the comma-separated patterns to exclude from auto-discovery.
    /// </summary>
    [CommandOption("--exclude-patterns")]
    [Description("Comma-separated patterns to exclude from auto-discovery.")]
    public string? ExcludePatterns { get; init; }
}
