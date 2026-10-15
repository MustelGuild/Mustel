using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Query.Run;

/// <summary>
/// Settings for the query run command.
/// </summary>
public sealed class QueryRunSettings : CommandSettings
{
    /// <summary>
    /// Gets the path to the SQL input file.
    /// </summary>
    [CommandOption("-i|--input")]
    [Description("Path to the SQL input file.")]
    public string? Input { get; init; }

    /// <summary>
    /// Gets the inline SQL query to execute (alternative to --input).
    /// </summary>
    [CommandOption("-c|--command")]
    [Description("Inline SQL query to execute (alternative to --input).")]
    public string? Command { get; init; }

    /// <summary>
    /// Gets the path to the output directory.
    /// </summary>
    [CommandOption("-o|--output")]
    [Description("Path to the output directory.")]
    public string? Output { get; init; }

    /// <summary>
    /// Gets the complete Npgsql connection string. When provided, bypasses server selection
    /// and executes the query directly against this connection. Individual connection options
    /// (--host, --port, etc.) can still override specific parameters within this string.
    /// </summary>
    [CommandOption("--npgsql-connection-string")]
    [Description("Complete Npgsql connection string. Bypasses server selection when provided.")]
    public string? NpgsqlConnectionString { get; init; }

    /// <summary>
    /// Gets the database host address. Overrides the server config host when provided.
    /// </summary>
    [CommandOption("-H|--host")]
    [Description("Database host address. Overrides the configured server host.")]
    public string? Host { get; init; }

    /// <summary>
    /// Gets the database port. Overrides the server config port when provided.
    /// </summary>
    [CommandOption("-p|--port")]
    [Description("Database port. Overrides the configured server port.")]
    public string? Port { get; init; }

    /// <summary>
    /// Gets the database name. Overrides the server config database when provided.
    /// </summary>
    [CommandOption("-d|--database")]
    [Description("Database name. Overrides the configured server database.")]
    public string? Database { get; init; }

    /// <summary>
    /// Gets the database username. Overrides the server config username when provided.
    /// </summary>
    [CommandOption("-U|--username")]
    [Description("Database username. Overrides the configured server username.")]
    public string? Username { get; init; }

    /// <summary>
    /// Gets the database password. Overrides the server config password when provided.
    /// </summary>
    [CommandOption("-W|--password")]
    [Description("Database password. Overrides the stored server password.")]
    public string? Password { get; init; }

    /// <summary>
    /// Gets the SSL mode. Overrides the server config SSL mode when provided.
    /// </summary>
    [CommandOption("--ssl-mode")]
    [Description("SSL mode (Disable, Allow, Prefer, Require, VerifyCA, VerifyFull). Default: Prefer.")]
    public string? SslMode { get; init; }

    /// <summary>
    /// Gets the connection timeout in seconds. Overrides the server config timeout when provided.
    /// </summary>
    [CommandOption("--timeout")]
    [Description("Connection timeout in seconds. Default: 30.")]
    public int? Timeout { get; init; }

    /// <summary>
    /// Gets the command timeout in seconds. Overrides the server config command timeout when provided.
    /// </summary>
    [CommandOption("--command-timeout")]
    [Description("Command timeout in seconds. Default: 300.")]
    public int? CommandTimeout { get; init; }

    /// <summary>
    /// Gets a value indicating whether to enable connection pooling.
    /// </summary>
    [CommandOption("--pooling")]
    [Description("Enable connection pooling. Default: true.")]
    public bool? Pooling { get; init; }

    /// <summary>
    /// Gets the minimum pool size.
    /// </summary>
    [CommandOption("--min-pool-size")]
    [Description("Minimum pool size. Default: 1.")]
    public int? MinPoolSize { get; init; }

    /// <summary>
    /// Gets the maximum pool size.
    /// </summary>
    [CommandOption("--max-pool-size")]
    [Description("Maximum pool size. Default: 100.")]
    public int? MaxPoolSize { get; init; }

    /// <summary>
    /// Gets the keepalive interval in seconds.
    /// </summary>
    [CommandOption("--keepalive")]
    [Description("Keepalive interval in seconds. Default: 0.")]
    public int? Keepalive { get; init; }

    /// <summary>
    /// Gets the connection lifetime in seconds.
    /// </summary>
    [CommandOption("--connection-lifetime")]
    [Description("Connection lifetime in seconds. Default: 0.")]
    public int? ConnectionLifetime { get; init; }

    /// <summary>
    /// Gets a value indicating whether to execute the query on all databases on the server.
    /// When set, forces <c>FetchAllDatabases</c> on all selected servers regardless of their configuration.
    /// </summary>
    [CommandOption("-a|--all")]
    [Description("Execute the query on all databases on the server.")]
    public bool All { get; init; }

    /// <summary>
    /// Gets the comma-separated list of database names to exclude from execution.
    /// These names are added to each server's configured exclude patterns.
    /// </summary>
    [CommandOption("--exclude")]
    [Description("Comma-separated list of database names to exclude.")]
    public string? Exclude { get; init; }

    /// <summary>
    /// Gets a value indicating whether to skip the confirmation prompt for destructive queries.
    /// </summary>
    [CommandOption("--no-confirm")]
    [Description("Skip confirmation prompt for destructive queries.")]
    public bool NoConfirm { get; init; }
}
