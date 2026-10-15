using System.Text.Json.Serialization;

namespace FurLab.CLI.Commands.WindowsFeatures;

/// <summary>
/// Represents the data model for a Windows features export.
/// </summary>
public class WindowsFeaturesExport
{
    /// <summary>
    /// Gets or sets the UTC timestamp when the export was created.
    /// </summary>
    [JsonPropertyName("exportedAt")]
    public DateTime ExportedAt { get; set; }

    /// <summary>
    /// Gets or sets the list of enabled Windows feature names.
    /// </summary>
    [JsonPropertyName("features")]
    public List<string> Features { get; set; } = [];
}
