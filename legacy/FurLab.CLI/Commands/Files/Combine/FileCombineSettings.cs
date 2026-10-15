using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Files.Combine;

/// <summary>
/// Settings for the file combine command.
/// </summary>
public sealed class FileCombineSettings : CommandSettings
{
    /// <summary>
    /// Gets the input file pattern (e.g., *.sql, *.txt).
    /// </summary>
    [CommandOption("-i|--input")]
    [Description("Input file pattern (e.g., *.sql, *.txt).")]
    public string Input { get; init; } = string.Empty;

    /// <summary>
    /// Gets the output file path.
    /// </summary>
    [CommandOption("-o|--output")]
    [Description("Output file path.")]
    public string? Output { get; init; }
}
