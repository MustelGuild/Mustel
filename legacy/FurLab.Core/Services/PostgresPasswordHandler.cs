using FurLab.Core.Interfaces;

namespace FurLab.Core.Services;

/// <summary>
/// Provides secure password handling for PostgreSQL operations.
/// </summary>
public class PostgresPasswordHandler : IPostgresPasswordHandler
{
    /// <inheritdoc/>
    public string ReadPasswordInteractively(string prompt)
    {
        if (!string.IsNullOrWhiteSpace(prompt))
        {
            Console.Write(prompt);
        }

        var password = string.Empty;
        var consoleKeyInfo = Console.ReadKey(true);

        while (consoleKeyInfo.Key != ConsoleKey.Enter)
        {
            if (consoleKeyInfo.Key == ConsoleKey.Backspace)
            {
                if (password.Length > 0)
                {
                    password = password.Substring(0, password.Length - 1);
                }
            }
            else if (!char.IsControl(consoleKeyInfo.KeyChar))
            {
                password += consoleKeyInfo.KeyChar;
            }

            consoleKeyInfo = Console.ReadKey(true);
        }

        Console.WriteLine();
        return password;
    }
}
