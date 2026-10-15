using FurLab.Core.Interfaces;
using FurLab.Core.Models;

using Spectre.Console;
using Spectre.Console.Cli;

namespace FurLab.CLI.Commands.Files.ConvertEncoding;

/// <summary>
/// Converts files from one encoding to another.
/// </summary>
/// <remarks>
/// Initializes a new instance of the <see cref="FilesConvertEncodingCommand"/> class.
/// </remarks>
/// <param name="conversionService">The encoding conversion service.</param>
public sealed class FilesConvertEncodingCommand(IEncodingConversionService conversionService) : AsyncCommand<FilesConvertEncodingSettings>
{
    private readonly IEncodingConversionService _conversionService = conversionService ?? throw new ArgumentNullException(nameof(conversionService));

    /// <inheritdoc/>
    protected override async Task<int> ExecuteAsync(CommandContext context, FilesConvertEncodingSettings settings, CancellationToken cancellationToken)
    {
        // Validate settings
        if (string.IsNullOrWhiteSpace(settings.Input))
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Input pattern is required. Use -i/--input to specify a file pattern.");
            return 2;
        }

        if (settings.ConfidenceThreshold is < 0.0 or > 1.0)
        {
            AnsiConsole.MarkupLine("[red]Error:[/] Confidence threshold must be between 0.0 and 1.0.");
            return 2;
        }

        // Validate input pattern for path traversal
        var baseInputPath = settings.Input.Replace("*", string.Empty).Replace("?", string.Empty);
        if (!string.IsNullOrEmpty(baseInputPath) && baseInputPath.Contains("..") && !SecurityUtils.IsValidPath(baseInputPath))
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] Invalid input pattern: '{settings.Input}'. Path traversal not allowed.");
            return 2;
        }

        try
        {
            // Validate output directory if specified
            if (!string.IsNullOrEmpty(settings.OutputDirectory))
            {
                if (!SecurityUtils.IsValidPath(settings.OutputDirectory))
                {
                    AnsiConsole.MarkupLine($"[red]Error:[/] Invalid output directory path: '{settings.OutputDirectory}'. Path traversal not allowed.");
                    return 2;
                }

                Directory.CreateDirectory(settings.OutputDirectory);
            }

            // Parse exclude patterns
            var excludePatterns = new List<string>();
            if (!string.IsNullOrEmpty(settings.ExcludePatterns))
            {
                excludePatterns.AddRange(settings.ExcludePatterns.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries));
            }

            // Build options
            var options = new EncodingConversionOptions
            {
                Pattern = settings.Input,
                SourceEncoding = settings.SourceEncoding,
                TargetEncoding = settings.TargetEncoding,
                OutputDirectory = settings.OutputDirectory,
                CreateBackup = settings.CreateBackup,
                TextOnly = settings.TextOnly,
                ConfidenceThreshold = settings.ConfidenceThreshold,
                Force = settings.Force,
                ExcludePatterns = excludePatterns,
            };

            // Show what we're doing
            AnsiConsole.MarkupLine($"[blue]Converting files matching:[/] {settings.Input}");
            AnsiConsole.MarkupLine($"[blue]Target encoding:[/] {settings.TargetEncoding}");

            if (!string.IsNullOrEmpty(settings.SourceEncoding))
            {
                AnsiConsole.MarkupLine($"[blue]Source encoding:[/] {settings.SourceEncoding}");
            }
            else
            {
                AnsiConsole.MarkupLine($"[blue]Source encoding:[/] Auto-detect (confidence threshold: {settings.ConfidenceThreshold:P0})");
            }

            if (settings.TextOnly)
            {
                AnsiConsole.MarkupLine("[blue]Filter:[/] Text files only");
            }

            if (settings.CreateBackup)
            {
                AnsiConsole.MarkupLine("[blue]Backup:[/] Enabled (.bak files will be created)");
            }

            AnsiConsole.WriteLine();

            // Progress reporter
            var progress = new Progress<EncodingConversionProgress>(p =>
            {
                AnsiConsole.MarkupLine($"[grey]{p.ProcessedFiles}/{p.TotalFiles}[/] {p.Message}");
            });

            // Execute conversion
            EncodingConversionResult result;

            if (AnsiConsole.Profile.Capabilities.Interactive)
            {
                result = await AnsiConsole.Status()
                    .StartAsync("Converting files...", async ctx => await _conversionService.ConvertFilesAsync(options, progress, cancellationToken));
            }
            else
            {
                result = await _conversionService.ConvertFilesAsync(options, progress, cancellationToken);
            }

            // Display results
            AnsiConsole.WriteLine();
            AnsiConsole.MarkupLine("[green]Conversion complete![/]");
            AnsiConsole.WriteLine();

            // Summary table
            var table = new Table();
            table.AddColumn("Metric");
            table.AddColumn("Count");

            table.AddRow("Total files", result.TotalFiles.ToString());
            table.AddRow("[green]Converted[/]", result.ConvertedCount.ToString());
            table.AddRow("[yellow]Skipped[/] (already in target encoding)", result.SkippedCount.ToString());
            table.AddRow("[red]Errors[/]", result.ErrorCount.ToString());

            AnsiConsole.Write(table);

            // Show errors if any
            if (result.Errors.Count > 0)
            {
                AnsiConsole.WriteLine();
                AnsiConsole.MarkupLine("[red]Errors:[/]");

                foreach (var error in result.Errors)
                {
                    AnsiConsole.MarkupLine($"  [red]•[/] {Path.GetFileName(error.FilePath)}: {error.ErrorMessage}");
                }
            }

            // Show converted files with details
            if (result.ProcessedFiles.Count > 0)
            {
                AnsiConsole.WriteLine();
                AnsiConsole.MarkupLine("[green]Converted files:[/]");

                var filesTable = new Table();
                filesTable.AddColumn("File");
                filesTable.AddColumn("From");
                filesTable.AddColumn("To");
                filesTable.AddColumn("Confidence");

                foreach (var file in result.ProcessedFiles.Take(20)) // Limit to 20 for display
                {
                    var confidenceText = file.DetectionConfidence switch
                    {
                        1.0 => "[green]100%[/]",
                        >= 0.8 => $"[green]{file.DetectionConfidence:P0}[/]",
                        >= 0.5 => $"[yellow]{file.DetectionConfidence:P0}[/]",
                        _ => $"[red]{file.DetectionConfidence:P0}[/]",
                    };

                    filesTable.AddRow(
                        Path.GetFileName(file.OriginalPath),
                        file.SourceEncoding,
                        file.TargetEncoding,
                        confidenceText);
                }

                if (result.ProcessedFiles.Count > 20)
                {
                    filesTable.AddRow($"... and {result.ProcessedFiles.Count - 20} more", "", "", "");
                }

                AnsiConsole.Write(filesTable);
            }

            // Return appropriate exit code
            return result.ErrorCount > 0 ? 1 : 0;
        }
        catch (OperationCanceledException)
        {
            AnsiConsole.MarkupLine("[yellow]Operation cancelled.[/]");
            return 130;
        }
        catch (Exception ex)
        {
            AnsiConsole.MarkupLine($"[red]Error:[/] {ex.Message}");
            return 1;
        }
    }
}
