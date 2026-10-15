using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.WindowsFeatures.Export;

/// <summary>
/// Settings for the windowsfeatures export command.
/// </summary>
public sealed class WindowsFeaturesExportSettings : CommandSettings
{
    /// <summary>
    /// Gets the path to save the exported features JSON file.
    /// </summary>
    [CommandArgument(0, "<path>")]
    [Description("Path to save the exported features JSON file.")]
    public string Path { get; init; } = string.Empty;
}
