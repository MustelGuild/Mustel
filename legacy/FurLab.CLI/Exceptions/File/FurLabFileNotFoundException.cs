namespace FurLab.CLI.Exceptions.File;

/// <summary>
/// Exception thrown when a file is not found.
/// </summary>
/// <remarks>
/// Initializes a new instance of the <see cref="FurLabFileNotFoundException"/> class.
/// </remarks>
/// <param name="filePath">The path to the file that was not found.</param>
public class FurLabFileNotFoundException(string filePath) : Exception($"File not found: {filePath}")
{
}
