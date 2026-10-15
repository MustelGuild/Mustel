using System.Diagnostics;
using System.Text.Json;

using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.WindowsFeatures.Export;

/// <summary>
/// Exports enabled Windows features to a JSON file.
/// </summary>
public sealed class WindowsFeaturesExportCommand : AsyncCommand<WindowsFeaturesExportSettings>
{
    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, WindowsFeaturesExportSettings settings, CancellationToken cancellation)
    {
        Console.WriteLine("Getting enabled Windows features...");

        var features = GetEnabledFeatures();

        var exportData = new WindowsFeaturesExport
        {
            ExportedAt = DateTime.UtcNow,
            Features = features
        };

        var json = JsonSerializer.Serialize(exportData, new JsonSerializerOptions
        {
            WriteIndented = true
        });

        File.WriteAllText(settings.Path, json);
        Console.WriteLine($"Exported {features.Count} enabled features to: {settings.Path}");

        return Task.FromResult(0);
    }

    private static List<string> GetEnabledFeatures()
    {
        var enabledFeatures = new List<string>();

        var process = new Process
        {
            StartInfo = new ProcessStartInfo
            {
                FileName = "dism.exe",
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                CreateNoWindow = true
            }
        };
        process.StartInfo.ArgumentList.Add("/online");
        process.StartInfo.ArgumentList.Add("/get-features");
        process.StartInfo.ArgumentList.Add("/format:list");

        process.Start();
        var output = process.StandardOutput.ReadToEnd();
        process.WaitForExit();

        var lines = output.Split('\n');
        foreach (var line in lines)
        {
            if (line.Contains("Enabled"))
            {
                var featureName = ExtractFeatureName(line);
                if (!string.IsNullOrEmpty(featureName))
                {
                    enabledFeatures.Add(featureName);
                }
            }
        }

        return enabledFeatures;
    }

    private static string? ExtractFeatureName(string line)
    {
        var startIndex = line.IndexOf("Feature Name :", StringComparison.OrdinalIgnoreCase);
        if (startIndex >= 0)
        {
            startIndex += "Feature Name :".Length;
            var endIndex = line.IndexOf('\r', startIndex);
            if (endIndex < 0)
            {
                endIndex = line.Length;
            }
            return line.Substring(startIndex, endIndex - startIndex).Trim();
        }
        return null;
    }
}
