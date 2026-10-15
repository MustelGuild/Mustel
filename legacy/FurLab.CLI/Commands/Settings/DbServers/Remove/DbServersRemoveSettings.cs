using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.Remove;

/// <summary>
/// Settings for the db-servers remove command.
/// </summary>
public sealed class DbServersRemoveSettings : CommandSettings
{
    /// <summary>
    /// Gets the server name to remove.
    /// </summary>
    [CommandOption("-n|--name")]
    [Description("Server name to remove.")]
    public string? Name { get; init; }
}
