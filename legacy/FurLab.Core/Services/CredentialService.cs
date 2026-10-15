using FurLab.Core.Interfaces;

using Microsoft.AspNetCore.DataProtection;

namespace FurLab.Core.Services;

/// <summary>
/// Encrypts and decrypts server passwords using <see cref="IDataProtectionProvider"/>.
/// On Windows, keys are protected at rest with DPAPI. On Linux/macOS, keys are stored
/// as XML files under the configured key ring directory.
/// </summary>
public sealed class CredentialService : ICredentialService
{
    private const string Purpose = "FurLab.ServerPasswords.v1";

    private readonly IDataProtector _protector;

    /// <summary>
    /// Initializes a new instance of the <see cref="CredentialService"/> class.
    /// </summary>
    /// <param name="provider">The data protection provider.</param>
    public CredentialService(IDataProtectionProvider provider)
    {
        ArgumentNullException.ThrowIfNull(provider);
        _protector = provider.CreateProtector(Purpose);
    }

    /// <inheritdoc/>
    public string Encrypt(string plaintext)
    {
        ArgumentNullException.ThrowIfNull(plaintext);
        return _protector.Protect(plaintext);
    }

    /// <inheritdoc/>
    public string? TryDecrypt(string? encrypted)
    {
        if (string.IsNullOrEmpty(encrypted))
        {
            return null;
        }

        try
        {
            return _protector.Unprotect(encrypted);
        }
        catch
        {
            return null;
        }
    }
}
