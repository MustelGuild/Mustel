using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.SetPassword;

/// <summary>
/// Settings for the db-servers set-password command.
/// </summary>
public sealed class DbServersSetPasswordSettings : CommandSettings
{
    /// <summary>
    /// Gets the server name. If omitted, displays interactive selection.
    /// </summary>
    [CommandArgument(0, "[name]")]
    [Description("Server name. If omitted, displays interactive selection.")]
    public string? Name { get; init; }
}
