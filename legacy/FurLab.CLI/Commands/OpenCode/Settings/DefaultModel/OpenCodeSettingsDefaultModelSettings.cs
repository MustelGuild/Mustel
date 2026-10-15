using System.ComponentModel;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.OpenCode.Settings.DefaultModel;

/// <summary>
/// Settings for the opencode default-model command.
/// </summary>
public sealed class OpenCodeSettingsDefaultModelSettings : CommandSettings
{
    /// <summary>
    /// Gets the model ID to set as default. If omitted, displays interactive menu.
    /// </summary>
    [CommandArgument(0, "[model-id]")]
    [Description("Model ID to set as default. If omitted, displays interactive menu.")]
    public string? ModelId { get; init; }

    /// <summary>
    /// Gets a value indicating whether to modify the global configuration instead of local.
    /// </summary>
    [CommandOption("-g|--global")]
    [Description("Changes the global configuration (~/.config/opencode/opencode.jsonc) instead of local.")]
    public bool Global { get; init; }
}
