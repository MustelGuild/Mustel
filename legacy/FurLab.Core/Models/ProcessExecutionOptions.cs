namespace FurLab.Core.Models;

/// <summary>
/// Defines options for process execution.
/// </summary>
public record ProcessExecutionOptions
{
    /// <summary>
    /// Gets or sets the file name or path of the executable.
    /// </summary>
    public string FileName { get; init; } = string.Empty;

    /// <summary>
    /// Gets or sets the command-line arguments.
    /// </summary>
    public List<string>? Arguments { get; init; }

    /// <summary>
    /// Gets or sets the working directory.
    /// </summary>
    public string? WorkingDirectory { get; init; }

    /// <summary>
    /// Gets or sets environment variables for the process.
    /// </summary>
    public Dictionary<string, string>? EnvironmentVariables { get; init; }

    /// <summary>
    /// Gets or sets whether to redirect standard output.
    /// </summary>
    public bool RedirectStandardOutput { get; init; } = true;

    /// <summary>
    /// Gets or sets whether to redirect standard error.
    /// </summary>
    public bool RedirectStandardError { get; init; } = true;

    /// <summary>
    /// Gets or sets whether to create a new window.
    /// </summary>
    public bool CreateNoWindow { get; init; } = true;
}
