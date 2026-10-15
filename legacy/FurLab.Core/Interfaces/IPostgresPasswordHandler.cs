namespace FurLab.Core.Interfaces;

/// <summary>
/// Provides secure password handling for PostgreSQL operations.
/// </summary>
public interface IPostgresPasswordHandler
{
    /// <summary>
    /// Reads a password from the console interactively without displaying the characters.
    /// </summary>
    /// <param name="prompt">The prompt message to display before reading the password.</param>
    /// <returns>The password entered by the user, or an empty string if no input was given.</returns>
    string ReadPasswordInteractively(string prompt);
}
