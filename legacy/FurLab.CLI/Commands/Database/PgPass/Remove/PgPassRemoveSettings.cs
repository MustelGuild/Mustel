using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Database.PgPass.Remove;

/// <summary>
/// Settings for the pgpass remove command.
/// </summary>
public sealed class PgPassRemoveSettings : CommandSettings
{
    /// <summary>
    /// Gets the database name of the entry to remove (or * for wildcard).
    /// </summary>
    [CommandArgument(0, "<database>")]
    [Description("Database name of the entry to remove (or * for wildcard).")]
    public string Database { get; init; } = string.Empty;

    /// <summary>
    /// Gets the hostname of the entry to remove.
    /// </summary>
    [CommandOption("--host")]
    [Description("Hostname of the entry to remove.")]
    public string? Host { get; init; }

    /// <summary>
    /// Gets the port of the entry to remove.
    /// </summary>
    [CommandOption("--port")]
    [Description("Port of the entry to remove.")]
    public string? Port { get; init; }

    /// <summary>
    /// Gets the username of the entry to remove.
    /// </summary>
    [CommandOption("--username")]
    [Description("Username of the entry to remove.")]
    public string? Username { get; init; }
}
