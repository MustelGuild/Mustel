using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Settings.DbServers.Test;

/// <summary>
/// Settings for the db-servers test command.
/// </summary>
public sealed class DbServersTestSettings : CommandSettings
{
    /// <summary>
    /// Gets the server name to test.
    /// </summary>
    [CommandOption("-n|--name")]
    [Description("Server name to test.")]
    public string? Name { get; init; }
}
