using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.WindowsFeatures.List;

/// <summary>
/// Settings for the windowsfeatures list command.
/// </summary>
public sealed class WindowsFeaturesListSettings : CommandSettings
{
    /// <summary>
    /// Gets a value indicating whether to show only enabled features.
    /// </summary>
    [CommandOption("--enabled-only")]
    [Description("Show only enabled features.")]
    public bool EnabledOnly { get; init; }
}
