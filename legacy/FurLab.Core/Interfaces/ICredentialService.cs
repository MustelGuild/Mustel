namespace FurLab.Core.Interfaces;

/// <summary>
/// Defines a service for encrypting and decrypting server credentials using the
/// application's data protection provider.
/// </summary>
public interface ICredentialService
{
    /// <summary>
    /// Encrypts a plaintext password and returns a base64 blob.
    /// </summary>
    /// <param name="plaintext">The plaintext password to encrypt.</param>
    /// <returns>An encrypted base64 string suitable for storage.</returns>
    string Encrypt(string plaintext);

    /// <summary>
    /// Attempts to decrypt an encrypted password blob.
    /// Returns <see langword="null"/> if decryption fails, the input is null, or the input is empty.
    /// Never throws.
    /// </summary>
    /// <param name="encrypted">The encrypted base64 blob, or null/empty.</param>
    /// <returns>The decrypted plaintext password, or <see langword="null"/> if unavailable.</returns>
    string? TryDecrypt(string? encrypted);
}
