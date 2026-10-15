using FurLab.Core.Models;

namespace FurLab.Core.Interfaces;

/// <summary>
/// Defines a service for converting file encodings.
/// </summary>
public interface IEncodingConversionService
{
    /// <summary>
    /// Converts files matching the specified pattern to the target encoding.
    /// </summary>
    /// <param name="options">The conversion options.</param>
    /// <param name="progress">Optional progress reporter.</param>
    /// <param name="cancellationToken">Optional cancellation token.</param>
    /// <returns>A task representing the operation with the conversion result.</returns>
    Task<EncodingConversionResult> ConvertFilesAsync(
        EncodingConversionOptions options,
        IProgress<EncodingConversionProgress>? progress = null,
        CancellationToken cancellationToken = default);
}
