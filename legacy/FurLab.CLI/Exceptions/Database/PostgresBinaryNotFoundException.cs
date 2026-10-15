namespace FurLab.CLI.Exceptions.Database;

/// <summary>
/// Exception thrown when a PostgreSQL binary is not found.
/// </summary>
/// <remarks>
/// Initializes a new instance of the <see cref="PostgresBinaryNotFoundException"/> class.
/// </remarks>
/// <param name="binaryName">The name of the binary that was not found.</param>
public class PostgresBinaryNotFoundException(string binaryName) : Exception($"PostgreSQL binary '{binaryName}' not found. Please ensure PostgreSQL is installed and the binaries are in your PATH.")
{
}
