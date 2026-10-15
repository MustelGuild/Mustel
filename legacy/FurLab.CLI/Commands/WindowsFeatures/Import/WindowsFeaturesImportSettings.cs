using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.WindowsFeatures.Import;

/// <summary>
/// Settings for the windowsfeatures import command.
/// </summary>
public sealed class WindowsFeaturesImportSettings : CommandSettings
{
    /// <summary>
    /// Gets the path to the JSON file containing exported features.
    /// </summary>
    [CommandArgument(0, "<path>")]
    [Description("Path to the JSON file containing exported features.")]
    public string Path { get; init; } = string.Empty;
}
