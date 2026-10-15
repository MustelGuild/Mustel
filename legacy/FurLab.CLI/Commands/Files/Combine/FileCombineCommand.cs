using System.Text;

using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Files.Combine;

/// <summary>
/// Combines files matching a pattern into a single output file.
/// </summary>
public sealed class FileCombineCommand : AsyncCommand<FileCombineSettings>
{
    /// <inheritdoc/>
    protected override Task<int> ExecuteAsync(CommandContext context, FileCombineSettings settings, CancellationToken cancellation)
    {
        if (string.IsNullOrWhiteSpace(settings.Input))
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Input pattern is required. Use -i/--input to specify a file pattern.");
            return Task.FromResult(2);
        }

        var pattern = Path.GetFileName(settings.Input);
        if (string.IsNullOrWhiteSpace(pattern))
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Input pattern is invalid.");
            return Task.FromResult(2);
        }

        var directory = Path.GetDirectoryName(settings.Input);
        if (string.IsNullOrWhiteSpace(directory))
        {
            directory = Environment.CurrentDirectory;
        }

        if (!SecurityUtils.IsValidPath(directory))
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] Invalid directory path: '{directory}'. Path traversal not allowed.");
            return Task.FromResult(2);
        }

        var fullDirectoryPath = Path.GetFullPath(directory);
        var extension = Path.GetExtension(settings.Input);
        var outputPath = settings.Output;

        if (string.IsNullOrWhiteSpace(outputPath))
        {
            outputPath = Path.Combine(directory, $"CombineFiles{extension}");
        }

        var fullOutputPath = Path.GetFullPath(outputPath);
        if (!SecurityUtils.IsValidPath(fullOutputPath, fullDirectoryPath))
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] Output path is outside the input directory: '{outputPath}'");
            return Task.FromResult(2);
        }

        var allFileText = new StringBuilder();
        var currentEncoding = Encoding.UTF8;

        var inputFilePaths = Directory.GetFiles(fullDirectoryPath, pattern);
        Console.WriteLine("Number of files: {0}.", inputFilePaths.Length);

        if (inputFilePaths.Length == 0)
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] No files found matching pattern '{pattern}' in directory '{fullDirectoryPath}'.");
            return Task.FromResult(2);
        }

        foreach (var inputFilePath in inputFilePaths)
        {
            currentEncoding = FileEncodingHelper.DetectEncoding(inputFilePath);
            allFileText.Append(File.ReadAllText(inputFilePath, currentEncoding));
            allFileText.AppendLine();

            Console.WriteLine("The file {0} has been processed.", inputFilePath);
        }

        File.WriteAllText(fullOutputPath, allFileText.ToString(), currentEncoding);
        return Task.FromResult(0);
    }
}
