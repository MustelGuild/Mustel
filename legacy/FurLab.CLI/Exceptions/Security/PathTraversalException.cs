namespace FurLab.CLI.Exceptions.Security;

/// <summary>
/// Exception thrown when a path traversal attempt is detected.
/// </summary>
/// <remarks>
/// Initializes a new instance of the <see cref="PathTraversalException"/> class.
/// </remarks>
/// <param name="path">The path that contains the traversal attempt.</param>
public class PathTraversalException(string path) : Exception($"Path traversal detected: {path}")
{
}
