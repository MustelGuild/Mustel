using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Database.PgPass.Add;

/// <summary>
/// Settings for the pgpass add command.
/// </summary>
public sealed class PgPassAddSettings : CommandSettings
{
    /// <summary>
    /// Gets the database name (or * for wildcard).
    /// </summary>
    [CommandOption("--database")]
    [Description("Database name (or * for wildcard).")]
    public string? Database { get; init; }

    /// <summary>
    /// Gets the hostname or IP address of the PostgreSQL server.
    /// </summary>
    [CommandOption("--host")]
    [Description("Hostname or IP address of the PostgreSQL server.")]
    public string? Host { get; init; }

    /// <summary>
    /// Gets the TCP port of the PostgreSQL server.
    /// </summary>
    [CommandOption("--port")]
    [Description("TCP port of the PostgreSQL server.")]
    public string? Port { get; init; }

    /// <summary>
    /// Gets the PostgreSQL username.
    /// </summary>
    [CommandOption("--username")]
    [Description("PostgreSQL username.")]
    public string? Username { get; init; }

    /// <summary>
    /// Gets the PostgreSQL password. If not provided, it will be prompted interactively.
    /// </summary>
    [CommandOption("--password")]
    [Description("PostgreSQL password. Prompted interactively if not provided.")]
    public string? Password { get; init; }
}
