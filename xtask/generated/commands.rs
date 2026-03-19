/// ```text
/// asinh [-human] stretch { [offset] [-clipmode=] }
/// ```
///
/// Stretches the image to show faint objects using an hyperbolic arcsin transformation. The mandatory argument **stretch**, typically between 1 and 1000, will give the strength of the stretch. The black point can be offset by providing an optional **offset** argument in the normalized pixel value of [0, 1]. Finally the option **-human** enables using human eye luminous efficiency weights to compute the luminance used to compute the stretch value for each pixel, instead of the simple mean of the channels pixel values. This stretch method preserves lightness from the L\*a\*b\* color space. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Asinh {}

impl Command for Asinh {
    fn name() -> &'static str {
        "asinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// autoghs [-linked] shadowsclip stretchamount [-b=] [-hp=] [-lp=] [-clipmode=]
/// ```
///
/// Application of the generalized hyperbolic stretch with a symmetry point SP defined as k.sigma from the median of each channel (the provided **shadowsclip** value is the k here and can be negative). By default, SP and the stretch are computed per channel; SP can be computed as a mean of image channels by passing **-linked**. The stretch amount **D** is provided in the second mandatory argument.
/// Implicit values of 13 for **B**, making it very focused on the SP brightness range, 0.7 for **HP**, 0 for **LP** are used but can be changed with the options of the same names. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Autoghs {}

impl Command for Autoghs {
    fn name() -> &'static str {
        "autoghs"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// autostretch [-linked] [shadowsclip [targetbg]]
/// ```
///
/// Auto-stretches the currently loaded image, with different parameters for each channel (unlinked) unless **-linked** is passed. Arguments are optional, **shadowclip** is the shadows clipping point, measured in sigma units from the main histogram peak (default is -2.8), **targetbg** is the target background value, giving a final brightness to the image, range [0, 1], default is 0.25. The default values are those used in the Auto-stretch rendering from the GUI.
///
/// Do not use the unlinked version after color calibration, it will alter the white balance
///
#[derive(Builder)]
pub struct Autostretch {}

impl Command for Autostretch {
    fn name() -> &'static str {
        "autostretch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// bg
/// ```
///
/// Returns the background level of the loaded image
///
#[derive(Builder)]
pub struct Bg {}

impl Command for Bg {
    fn name() -> &'static str {
        "bg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// bgnoise
/// ```
///
/// Returns the background noise level of the loaded image
///
/// For more information, see the :ref:`statistics documentation
/// <Statistics:Background noise>`
///
#[derive(Builder)]
pub struct Bgnoise {}

impl Command for Bgnoise {
    fn name() -> &'static str {
        "bgnoise"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// binxy coefficient [-sum]
/// ```
///
/// Computes the numerical binning of the in-memory image (sum of the pixels 2x2, 3x3..., like the analogic binning of CCD camera). If the optional argument **-sum** is passed, then the sum of pixels is computed, while it is the average when no optional argument is provided
///
#[derive(Builder)]
pub struct Binxy {}

impl Command for Binxy {
    fn name() -> &'static str {
        "binxy"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// boxselect [-clear] [x y width height]
/// ```
///
/// Make a selection area in the currently loaded image with the arguments **x**, **y**, **width** and **height**, with **x** and **y** being the coordinates of the top left corner starting at (0, 0), and **width** and **height**, the size of the selection. The **-clear** argument deletes any selection area. If no argument is passed, the current selection is printed
///
#[derive(Builder)]
pub struct Boxselect {}

impl Command for Boxselect {
    fn name() -> &'static str {
        "boxselect"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// calibrate sequencename [-bias=filename] [-dark=filename] [-flat=filename] [-cc=dark [siglo sighi] || -cc=bpm bpmfile] [-cfa] [-debayer] [-fix_xtrans] [-equalize_cfa] [-opt[=exp]] [-all] [-prefix=] [-fitseq]
/// ```
///
/// Calibrates the sequence **sequencename** using bias, dark and flat given in argument.
///
/// For bias, a uniform level can be specified instead of an image, by entering a quoted expression starting with an = sign, such as -bias="=256" or -bias="=64*$OFFSET".
///
/// By default, cosmetic correction is not activated. If you wish to apply some, you will need to specify it with **-cc=** option.
/// You can use **-cc=dark** to detect hot and cold pixels from the masterdark (a masterdark must be given with the **-dark=** option), optionally followed by **siglo** and **sighi** for cold and hot pixels respectively. A value of 0 deactivates the correction. If sigmas are not provided, only hot pixels detection with a sigma of 3 will be applied.
/// Alternatively, you can use **-cc=bpm** followed by the path to your Bad Pixel Map to specify which pixels must be corrected. An example file can be obtained with a *find_hot* command on a masterdark.
///
/// Three options apply to color images (in CFA format): **-cfa** for cosmetic correction purposes, **-debayer** to demosaic images before saving them, and **-equalize_cfa** to equalize the mean intensity of RGB layers of the master flat, to avoid tinting the calibrated image.
/// The **-fix_xtrans** option is dedicated to X-Trans images by applying a correction on darks and biases to remove a rectangle pattern caused by autofocus.
/// It's also possible to optimize dark subtraction with **-opt**, which requires the supply of bias and dark masters, and automatically calculates the coefficient to be applied to dark, or calculates the coefficient thanks to the exposure keyword with **-opt=exp**.
/// By default, frames marked as excluded will not be processed. The argument **-all** can be used to force processing of all frames even if marked as excluded.
/// The output sequence name starts with the prefix "pp\_" unless otherwise specified with option **-prefix=**.
/// If **-fitseq** is provided, the output sequence will be a FITS sequence (single file)
///
#[derive(Builder)]
pub struct Calibrate {}

impl Command for Calibrate {
    fn name() -> &'static str {
        "calibrate"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// calibrate_single imagename [-bias=filename] [-dark=filename] [-flat=filename] [-cc=dark [siglo sighi] || -cc=bpm bpmfile] [-cfa] [-debayer] [-fix_xtrans] [-equalize_cfa] [-opt[=exp]] [-prefix=]
/// ```
///
/// Calibrates the image **imagename** using bias, dark and flat given in argument.
///
/// For bias, a uniform level can be specified instead of an image, by entering a quoted expression starting with an = sign, such as -bias="=256" or -bias="=64*$OFFSET".
///
/// By default, cosmetic correction is not activated. If you wish to apply some, you will need to specify it with **-cc=** option.
/// You can use **-cc=dark** to detect hot and cold pixels from the masterdark (a masterdark must be given with the **-dark=** option), optionally followed by **siglo** and **sighi** for cold and hot pixels respectively. A value of 0 deactivates the correction. If sigmas are not provided, only hot pixels detection with a sigma of 3 will be applied.
/// Alternatively, you can use **-cc=bpm** followed by the path to your Bad Pixel Map to specify which pixels must be corrected. An example file can be obtained with a *find_hot* command on a masterdark.
///
/// Three options apply to color images (in CFA format): **-cfa** for cosmetic correction purposes, **-debayer** to demosaic images before saving them, and **-equalize_cfa** to equalize the mean intensity of RGB layers of the master flat, to avoid tinting the calibrated image.
/// The **-fix_xtrans** option is dedicated to X-Trans images by applying a correction on darks and biases to remove a rectangle pattern caused by autofocus.
/// It's also possible to optimize dark subtraction with **-opt**, which requires the supply of bias and dark masters, and automatically calculates the coefficient to be applied to dark, or calculates the coefficient thanks to the exposure keyword with **-opt=exp**
/// The output filename starts with the prefix "pp\_" unless otherwise specified with option **-prefix=**
///
#[derive(Builder)]
pub struct CalibrateSingle {}

impl Command for CalibrateSingle {
    fn name() -> &'static str {
        "calibrate_single"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// capabilities
/// ```
///
/// Lists Siril capabilities, based on compilation options and runtime
///
#[derive(Builder)]
pub struct Capabilities {}

impl Command for Capabilities {
    fn name() -> &'static str {
        "capabilities"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// catsearch name
/// ```
///
/// Searches an object by **name** and adds it to the user annotation catalog. The object is first searched in the annotation catalogs, if not found a request is made to SIMBAD.
/// The object can be a solar system object, in which case a prefix, 'a:' for asteroid, 'p:' for planet, 'c:' for comet, 'dp:' for dwarf planet or 's:' for natural satellite, is required before the object name. The search is done for the date, time and observing location found in the image header, using the `IMCCE Miriade service <https://ssp.imcce.fr/webservices/miriade/howto/ephemcc/#howto-sso>`__
///
#[derive(Builder)]
pub struct Catsearch {}

impl Command for Catsearch {
    fn name() -> &'static str {
        "catsearch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// ccm m00 m01 m02 m10 m11 m12 m20 m21 m22 [gamma]
/// ```
///
/// Applies a color conversion matrix to the current image.
///
/// There are 9 mandatory arguments corresponding to the 9 matrix elements:
///
/// m00, m01, m02
/// m10, m11, m12
/// m20, m21, m22
///
/// An additional tenth argument **[gamma]** can be provided: if it is omitted, it defaults to 1.0.
///
/// These are applied to each pixel according to the following formulae:
///
/// r' = (m00 \* r + m01 \* g + m02 \* b)^(-1/gamma)
/// g' = (m10 \* r + m11 \* g + m12 \* b)^(-1/gamma)
/// b' = (m20 \* r + m21 \* g + m22 \* b)^(-1/gamma)
///
#[derive(Builder)]
pub struct Ccm {}

impl Command for Ccm {
    fn name() -> &'static str {
        "ccm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// cd directory
/// ```
///
/// Sets the new current working directory.
///
/// The argument **directory** can contain the ~ token, expanded as the home directory, directories with spaces in the name can be protected using single or double quotes
///
#[derive(Builder)]
pub struct Cd {}

impl Command for Cd {
    fn name() -> &'static str {
        "cd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// cdg
/// ```
///
/// Returns the coordinates of the center of gravity of the image. Only pixels with values above 15.7% of max ADU and having four neighbors filling the same condition are used to compute it, and it is computed only if there are at least 50 of them
///
#[derive(Builder)]
pub struct Cdg {}

impl Command for Cdg {
    fn name() -> &'static str {
        "cdg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// clahe cliplimit tileSize
/// ```
///
/// Equalizes the histogram of an image using Contrast Limited Adaptive Histogram Equalization.
///
/// **cliplimit** sets the threshold for contrast limiting.
/// **tilesize** sets the size of grid for histogram equalization. Input image will be divided into equally sized rectangular tiles
///
#[derive(Builder)]
pub struct Clahe {}

impl Command for Clahe {
    fn name() -> &'static str {
        "clahe"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// close
/// ```
///
/// Properly closes the opened image and the opened sequence, if any
///
#[derive(Builder)]
pub struct Close {}

impl Command for Close {
    fn name() -> &'static str {
        "close"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// conesearch [limit_magnitude] [-cat=] [-phot] [-obscode=] [-tag={on|off}] [-log={on|off}] [-trix=] [-out=]
/// ```
///
/// Displays stars from the local catalog by default for the loaded plate solved image, down to the provided **limit_magnitude** (13 by default for most catalogues, except 14.5 for aavso_chart, 20 for solsys, and omitted for pgc).
/// An alternate online catalog can be specified with **-cat=**, taking values
/// - for stars: tycho2, nomad, gaia, localgaia, ppmxl, bsc, apass, gcvs, vsx, simbad, aavso_chart
/// - for exoplanets: exo
/// - for deep-sky: pgc
/// - for solar system objects: solsys (closest `IAU observatory code <https://vo.imcce.fr/webservices/data/displayIAUObsCodes.php>`__ can be passed with the argument **-obscode=** for better position accuracy)
///
/// For stars catalogues containing photometric data, stars with no B-V information will be kept; they can be excluded by passing **-phot**
/// The argument **-trix=** can be passed instead of a catalogue followed by a number between 0 and 511 to plot stars contained in local catalogues trixel of level 3 (for dev usage mainly)
///
/// Some catalogs (bsc, gcvs, pgc, exo, aavso_chart, varisum and solsys) will also display, by default, names alongside markers in the display (GUI only) and list them in the log. For others with larger number of objects, namely vsx and simbad, the information can also be shown but, as it may clutter the display, it is not activated by default. This behavior can be toggled on/off with the options **-tag=on|off** to display names alongside markers and **-log=on|off** to list the objects in the console log
///
/// The list of items that are present in the image can optionally saved to a csv file by passing the argument **-out=**
///
#[derive(Builder)]
pub struct Conesearch {}

impl Command for Conesearch {
    fn name() -> &'static str {
        "conesearch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// convert basename [-debayer] [-fitseq] [-ser] [-start=index] [-out=]
/// ```
///
/// Converts all images of the current working directory that are in a supported format into Siril's sequence of FITS images (several files) or a FITS sequence (single file) if **-fitseq** is provided or a SER sequence (single file) if **-ser** is provided. The argument **basename** is the base name of the new sequence, numbers and the extension will be put behind it.
/// For FITS images, Siril will try to make a symbolic link; if not possible, files will be copied. The option **-debayer** applies demosaicing to CFA input images; in this case no symbolic link is done.
/// **-start=index** sets the starting index number, useful to continue an existing sequence (not used with -fitseq or **-ser**; make sure you remove or clear the target .seq if it exists in that case).
/// The **-out=** option changes the output directory to the provided argument.
///
/// See also CONVERTRAW and LINK
///
/// Links: :ref:`convertraw <convertraw>`, :ref:`link <link>`
///
#[derive(Builder)]
pub struct Convert {}

impl Command for Convert {
    fn name() -> &'static str {
        "convert"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// convertraw basename [-debayer] [-fitseq] [-ser] [-start=index] [-out=]
/// ```
///
/// Same as CONVERT but converts only DSLR RAW files found in the current working directory
///
/// Links: :ref:`convert <convert>`
///
#[derive(Builder)]
pub struct Convertraw {}

impl Command for Convertraw {
    fn name() -> &'static str {
        "convertraw"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// cosme [filename].lst
/// ```
///
/// Applies the local mean to a set of pixels on the loaded image (cosmetic correction). The coordinates of these pixels are in a text file [.lst file], the FIND_HOT command can also create it for single hot pixels, but manual operation is needed to remove rows or columns. COSME is adapted to correct residual hot and cold pixels after calibration.
/// Instead of providing the list of bad pixels, it's also possible to detect them in the current image using the FIND_COSME command
///
/// Links: :ref:`find_hot <find_hot>`, :ref:`find_cosme <find_cosme>`
///
/// File format for the bad pixels list:
/// * Lines in the form `P x y` will fix the pixel at coordinates (x, y) type is an optional character (C or H) specifying to Siril if the current pixel is cold or hot. This line is created by the command FIND_HOT but you also can add the two following line types manually
/// * Lines in the form `C x 0` will fix the bad column at coordinates x.
/// * Lines in the form `L y 0` will fix the bad line at coordinates y.
///
#[derive(Builder)]
pub struct Cosme {}

impl Command for Cosme {
    fn name() -> &'static str {
        "cosme"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// cosme_cfa [filename].lst
/// ```
///
/// Same function as COSME but applying to RAW CFA images
///
/// Links: :ref:`cosme <cosme>`
///
#[derive(Builder)]
pub struct CosmeCfa {}

impl Command for CosmeCfa {
    fn name() -> &'static str {
        "cosme_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// crop [x y width height]
/// ```
///
/// Crops to a selected area of the loaded image.
///
/// If a selection is active, no further arguments are required. Otherwise, or in scripts, arguments have to be given, with **x** and **y** being the coordinates of the top left corner, and **width** and **height** the size of the selection. Alternatively, the selection can be made using the BOXSELECT command
///
/// Links: :ref:`boxselect <boxselect>`
///
#[derive(Builder)]
pub struct Crop {}

impl Command for Crop {
    fn name() -> &'static str {
        "crop"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// denoise [-nocosmetic] [-mod=m] [ -vst | -da3d | -sos=n [-rho=r] ] [-indep]
/// ```
///
/// Denoises the image using the non-local Bayesian algorithm described by `Lebrun, Buades and Morel <https://www.ipol.im/pub/art/2013/16>`__.
///
/// It is strongly recommended to apply cosmetic correction to remove salt and pepper noise before running denoise, and by default this command will apply cosmetic correction automatically. However, if this has already been carried out earlier in the workflow it may be disabled here using the optional command **-nocosmetic**.
///
/// An optional argument **-mod=m** may be given, where 0 <= m <= 1. The output pixel is computed as : *out=m x d + (1 − m) x in*, where *d* is the denoised pixel value. A modulation value of 1 will apply no modulation. If the parameter is omitted, it defaults to 1.
///
/// The optional argument **-vst** can be used to apply the generalised Anscombe variance stabilising transform prior to NL-Bayes. This is useful with photon-starved images such as single subs, where the noise follows a Poisson or Poisson-Gaussian distribution rather than being primarily Gaussian. It cannot be used in conjunction with DA3D or SOS, and for denoising stacked images it is usually not beneficial.
///
/// The optional argument **-da3d** can be used to enable Data-Adaptive Dual Domain Denoising (DA3D) as a final stage denoising algorithm. This uses the output of BM3D as a guide image to refine the denoising. It improves detail and reduces staircasing artefacts.
///
/// The optional argument **-sos=\ n** can be used to enable Strengthen-Operate-Subtract (SOS) iterative denoise boosting, with the number of iterations specified by n. In particular, this booster may produce better results if the un-boosted NL-Bayes algorithm produces artefacts in background areas. If both -da3d and -sos=n are specified, the last to be specified will apply.
///
/// The optional argument **-rho=r** may be specified, where 0 < r < 1. This is used by the SOS booster to determine the amount of noisy image added in to the intermediate result between each iteration. If -sos=n is not specified then the parameter is ignored.
///
/// The default is not to apply DA3D or SOS, as the improvement in denoising is usually relatively small and these techniques requires additional processing time.
///
/// In very rare cases, blocky coloured artefacts may be found in the output when denoising colour images. The optional argument **-indep** can be used to prevent this by denoising each channel separately. This is slower but will eliminate artefacts
///
#[derive(Builder)]
pub struct Denoise {}

impl Command for Denoise {
    fn name() -> &'static str {
        "denoise"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// dumpheader
/// ```
///
/// Dumps the FITS header of the loaded image in the console
///
#[derive(Builder)]
pub struct Dumpheader {}

impl Command for Dumpheader {
    fn name() -> &'static str {
        "dumpheader"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// entropy
/// ```
///
/// Computes the entropy of the loaded image on the displayed layer, only in the selected area if one has been selected or in the whole image. The entropy is one way of measuring the noise or the details in an image
///
#[derive(Builder)]
pub struct Entropy {}

impl Command for Entropy {
    fn name() -> &'static str {
        "entropy"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// epf [-guided] [-d=] [-si=] [-ss=] [-mod=] [-guideimage=]
/// ```
///
/// Applies an edge preserving filter. By default a bilateral filter is applied; a guided filter can be specified using the argument **-guided**. The filter diameter defaults to 3 and can be set using **-d=**. Be careful with values of d greater than 20 as the algorithm can be computationally expensive.
///
/// The intensity filtering sigma value can be set using **-si=** and the spatial sigma value can be set using **-ss=**. Sigma values represent the difference in pixel values over which the filter acts strongly: for 32-bit images the value should be between 0 and 1.0, whereas for 16-bit images it should be between 0 and 65535. The defaults if not specified are for both to be set to 11. If **-d=0** is set then the filter diameter will be set automatically based on the value of **-ss**. *Note that when applying a guided filter, only* **-sc** *applies.*
///
/// When specifying a guided filter, a guide image may be set using **-guideimage=**. The default if no guide image is specified is to perform a self-guided filter. *Note: the guide image must have the same dimensions as the image to be filtered!*
///
/// The strength of the filter can be modulated using the **-mod=** argument. If mod = 1.0 the full effect of the filter will be applied; for mod less than 1.0 a proportion of the original image will be mixed with the result, and for mod = 0.0 no filtering will be applied
///
#[derive(Builder)]
pub struct Epf {}

impl Command for Epf {
    fn name() -> &'static str {
        "epf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// exit
/// ```
///
/// Quits the application
///
#[derive(Builder)]
pub struct Exit {}

impl Command for Exit {
    fn name() -> &'static str {
        "exit"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// extract NbPlans
/// ```
///
/// Extracts **NbPlans** planes of wavelet domain of the loaded image.
/// See also WAVELET and WRECONS. For color extraction, see SPLIT
///
/// Links: :ref:`wavelet <wavelet>`, :ref:`wrecons <wrecons>`, :ref:`split <split>`
///
#[derive(Builder)]
pub struct Extract {}

impl Command for Extract {
    fn name() -> &'static str {
        "extract"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// extract_Green
/// ```
///
/// Extracts green signal from the loaded CFA image. It reads the Bayer matrix information from the image or the preferences and exports only the averaged green filter data as a new half-sized FITS file. A new file is created, its name is prefixed with "Green\_"
///
#[derive(Builder)]
pub struct ExtractGreen {}

impl Command for ExtractGreen {
    fn name() -> &'static str {
        "extract_Green"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// extract_Ha [-upscale]
/// ```
///
/// Extracts H-Alpha signal from the loaded CFA image. It reads the Bayer matrix information from the image or the preferences and exports only the red filter data as a new half-sized FITS file. If the argument **-upscale** is provided, the output will be upscaled x2 to match the full sensor resolution, for example to match other images produced by the same family of sensors. A new file is created, its name is prefixed with "Ha\_"
///
#[derive(Builder)]
pub struct ExtractHa {}

impl Command for ExtractHa {
    fn name() -> &'static str {
        "extract_Ha"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// extract_HaOIII [-resample=]
/// ```
///
/// Extracts H-Alpha and O-III signals from the loaded CFA image. It reads the Bayer matrix information from the image or the preferences and exports only the red filter data for H-Alpha as a new half-sized FITS file (like EXTRACTHA) and keeps the three others for O-III with an interpolated replacement for the red pixel. The output files names start with the prefix "Ha\_" and "OIII\_"
///
/// The optional argument **-resample={ha|oiii}** sets whether to upsample the Ha image or downsample the OIII image to have images the same size. If this argument is not provided, no resampling will be carried out and the OIII image will have twice the height and width of the Ha image
///
#[derive(Builder)]
pub struct ExtractHaOIII {}

impl Command for ExtractHaOIII {
    fn name() -> &'static str {
        "extract_HaOIII"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// fdiv filename scalar
/// ```
///
/// Divides the loaded image by the image given in argument. The resulting image is multiplied by the value of the **scalar** argument. See also IDIV
///
/// Links: :ref:`idiv <idiv>`
///
#[derive(Builder)]
pub struct Fdiv {}

impl Command for Fdiv {
    fn name() -> &'static str {
        "fdiv"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// ffill value [x y width height]
/// ```
///
/// Same command as FILL but this is a symmetric fill of a region defined by the mouse or with BOXSELECT. Used to process an image in the Fourier (FFT) domain
///
/// Links: :ref:`fill <fill>`, :ref:`boxselect <boxselect>`
///
#[derive(Builder)]
pub struct Ffill {}

impl Command for Ffill {
    fn name() -> &'static str {
        "ffill"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// fftd modulus phase
/// ```
///
/// Applies a Fast Fourier Transform to the loaded image. **modulus** and **phase** given in argument are the names of the saved in FITS files
///
#[derive(Builder)]
pub struct Fftd {}

impl Command for Fftd {
    fn name() -> &'static str {
        "fftd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// ffti modulus phase
/// ```
///
/// Retrieves corrected image applying an inverse transformation. The **modulus** and **phase** arguments are the input file names, the result will be the new loaded image
///
#[derive(Builder)]
pub struct Ffti {}

impl Command for Ffti {
    fn name() -> &'static str {
        "ffti"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// fill value [x y width height]
/// ```
///
/// Fills the loaded image entirely or only the selection if there is one with pixels having the **value** intensity expressed in ADU
///
#[derive(Builder)]
pub struct Fill {}

impl Command for Fill {
    fn name() -> &'static str {
        "fill"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// find_cosme cold_sigma hot_sigma
/// ```
///
/// Applies an automatic detection and replacement of cold and hot pixels in the loaded image, with the thresholds passed in arguments in sigma units
///
#[derive(Builder)]
pub struct FindCosme {}

impl Command for FindCosme {
    fn name() -> &'static str {
        "find_cosme"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// find_cosme_cfa cold_sigma hot_sigma
/// ```
///
/// Same command as FIND_COSME but for CFA images
///
/// Links: :ref:`find_cosme <find_cosme>`
///
#[derive(Builder)]
pub struct FindCosmeCfa {}

impl Command for FindCosmeCfa {
    fn name() -> &'static str {
        "find_cosme_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// find_hot filename cold_sigma hot_sigma
/// ```
///
/// Saves a list file **filename** (text format) in the working directory which contains the coordinates of the pixels which have an intensity **hot_sigma** times higher and **cold_sigma** lower than standard deviation, extracted from the loaded image. We generally use this command on a master-dark file. The COSME command can apply this list of bad pixels to a loaded image, see also SEQCOSME to apply it to a sequence
///
/// Links: :ref:`cosme <cosme>`, :ref:`seqcosme <seqcosme>`
///
/// |
/// Lines ``P x y type`` will fix the pixel at coordinates (x, y) type is an optional character (C or H) specifying to Siril if the current pixel is cold or hot. This line is created by the command FIND_HOT but you also can add some lines manually:
/// Lines ``C x 0 type`` will fix the bad column at coordinates x.
/// Lines ``L y 0 type`` will fix the bad line at coordinates y.
///
#[derive(Builder)]
pub struct FindHot {}

impl Command for FindHot {
    fn name() -> &'static str {
        "find_hot"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// findcompstars star_name [-narrow|-wide] [-catalog={nomad|apass}] [-dvmag=3] [-dbv=0.5] [-emag=0.03] [-out=nina_file.csv]
/// ```
///
/// Automatically finds comparison stars in the field of the plate solved loaded image, for photometric analysis of a star's light curve according to
/// - the provided name of the star
/// - the field of view of the image, reduced to a diameter of its height if **-narrow** is passed, avoiding stars in the corners
/// - the chosen catalog (APASS by default), can be changed with **-catalog={NOMAD|APASS}**
/// - the difference in visual magnitude from the variable star, in the range [0, 6] with a default of 3, changed with **-dvmag=**
/// - the difference in color with the variable star, in the range [0.0, 0.7] of their B-V indices with a default of 0.5, changed with **-dbv=**
/// - the maximum allowed error on Vmag in the range [0.0, 0.1] with a default of 0.03, changed with **-emag=**.
///
/// The list can optionally be saved as a CSV file compatible with the NINA comparison stars list, specifying the file name with **-out=**. If the provided name is the special value **auto**, it is generated using the input parameters
///
/// See also LIGHT_CURVE
///
/// Links: :ref:`light_curve <light_curve>`
///
#[derive(Builder)]
pub struct Findcompstars {}

impl Command for Findcompstars {
    fn name() -> &'static str {
        "findcompstars"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// findstar [-out=] [-layer=] [-maxstars=]
/// ```
///
/// Detects stars in the currently loaded image, having a level greater than a threshold computed by Siril.
/// After that, a PSF is applied and Siril rejects all detected structures that don't fulfill a set of prescribed detection criteria, that can be tuned with command SETFINDSTAR.
/// Finally, an ellipse is drawn around detected stars.
///
/// Optional parameter **-out=** allows the results to be saved to the given path.
/// Option **-layer=** specifies the layer onto which the detection is performed (for color images only).
/// You can also limit the maximum number of stars detected by passing a value to option **-maxstars=**.
///
///
/// See also CLEARSTAR
///
/// Links: :ref:`psf <psf>`, :ref:`setfindstar <setfindstar>`, :ref:`clearstar <clearstar>`
///
#[derive(Builder)]
pub struct Findstar {}

impl Command for Findstar {
    fn name() -> &'static str {
        "findstar"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// fix_xtrans
/// ```
///
/// Fixes the Fujifilm X-Trans Auto Focus pixels in the loaded image.
///
/// Indeed, because of the phase detection auto focus system, the photosites used for auto focus get a little less light than the surrounding photosites. The camera compensates for this and increases the values from these specific photosites giving a visible square in the middle of the dark/bias frames
///
#[derive(Builder)]
pub struct FixXtrans {}

impl Command for FixXtrans {
    fn name() -> &'static str {
        "fix_xtrans"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// fixbanding amount sigma [-vertical]
/// ```
///
/// Tries to remove the horizontal or vertical banding in the loaded image.
/// **amount** defines the amount of correction, between 0 and 4.
/// **sigma** defines the highlight protection level of the algorithm, higher sigma gives higher protection, between 0 and 5. Values of 1 and 1 are often good enough.
/// **-vertical** option enables to perform vertical banding removal, horizontal is the default
///
#[derive(Builder)]
pub struct Fixbanding {}

impl Command for Fixbanding {
    fn name() -> &'static str {
        "fixbanding"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// fmedian ksize modulation
/// ```
///
/// Performs a median filter of size **ksize** x **ksize** (**ksize** MUST be odd) to the loaded image with a modulation parameter **modulation**.
///
/// The output pixel is computed as : out=mod x m + (1 − mod) x in, where m is the median-filtered pixel value. A modulation's value of 1 will apply no modulation
///
#[derive(Builder)]
pub struct Fmedian {}

impl Command for Fmedian {
    fn name() -> &'static str {
        "fmedian"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// fmul scalar
/// ```
///
/// Multiplies the loaded image by the **scalar** given in argument
///
#[derive(Builder)]
pub struct Fmul {}

impl Command for Fmul {
    fn name() -> &'static str {
        "fmul"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// gauss sigma
/// ```
///
/// Applies to the loaded image a Gaussian blur with the given **sigma**.
///
/// See also UNSHARP, the same with a blending parameter
///
/// Links: :ref:`unsharp <unsharp>`
///
#[derive(Builder)]
pub struct Gauss {}

impl Command for Gauss {
    fn name() -> &'static str {
        "gauss"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// get { -a | -A | variable }
/// ```
///
/// Gets a value from the settings using its name, or list all with **-a** (name and value list) or with **-A** (detailed list)
///
/// See also SET to update values
///
/// Links: :ref:`set <set>`
///
#[derive(Builder)]
pub struct Get {}

impl Command for Get {
    fn name() -> &'static str {
        "get"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// getref sequencename
/// ```
///
/// Prints information about the reference image of the sequence given in argument. First image has index 0
///
#[derive(Builder)]
pub struct Getref {}

impl Command for Getref {
    fn name() -> &'static str {
        "getref"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// ght -D= [-B=] [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels]
/// ```
///
/// Generalised hyperbolic stretch based on the work of the ghsastro.co.uk team.
///
/// The argument **-D=** defines the strength of the stretch, between 0 and 10. This is the only mandatory argument. The following optional arguments further tailor the stretch:
/// **B** defines the intensity of the stretch near the focal point, between -5 and 15;
/// **LP** defines a shadow preserving range between 0 and SP where the stretch will be linear, preserving shadow detail;
/// **SP** defines the symmetry point of the stretch, between 0 and 1, which is the point at which the stretch will be most intense;
/// **HP** defines a region between HP and 1 where the stretch is linear, preserving highlight details and preventing star bloat.
/// If omitted B, LP and SP default to 0.0 ad HP defaults to 1.0.
/// An optional argument (either **-human**, **-even** or **-independent**) can be passed to select either human-weighted or even-weighted luminance or independent colour channels for colour stretches. The argument is ignored for mono images. Alternatively, the argument **-sat** specifies that the stretch is performed on image saturation - the image must be color and all channels must be selected for this to work.
/// Optionally the parameter **[channels]** may be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Ght {}

impl Command for Ght {
    fn name() -> &'static str {
        "ght"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// grey_flat
/// ```
///
/// Equalizes the mean intensity of RGB layers in the loaded CFA image. This is the same process used on flats during calibration when the option equalize CFA is used
///
#[derive(Builder)]
pub struct GreyFlat {}

impl Command for GreyFlat {
    fn name() -> &'static str {
        "grey_flat"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// help [command]
/// ```
///
/// Lists the available commands or help for one command
///
#[derive(Builder)]
pub struct Help {}

impl Command for Help {
    fn name() -> &'static str {
        "help"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// histo channel (channel=0, 1, 2 with 0: red, 1: green, 2: blue)
/// ```
///
/// Calculates the histogram of the **layer** of the loaded image and produces file histo\_[channel name].dat in the working directory.
/// layer = 0, 1 or 2 with 0=red, 1=green and 2=blue
///
#[derive(Builder)]
pub struct Histo {}

impl Command for Histo {
    fn name() -> &'static str {
        "histo"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// iadd filename
/// ```
///
/// Adds the image **filename** to the loaded image.
/// Result will be in 32 bits per channel if allowed in the preferences
///
#[derive(Builder)]
pub struct Iadd {}

impl Command for Iadd {
    fn name() -> &'static str {
        "iadd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// icc_assign profile
/// ```
///
/// Assigns the ICC profile specified in the argument to the current image.
/// One of the following special arguments may be provided to use the respective built-in profiles: **sRGB**, **sRGBlinear**, **Rec2020**, **Rec2020linear**, **working** to set the working mono or RGB color profile, (for mono images only) **linear**, or the path to an ICC profile file may be provided. If a built-in profile is specified with a monochrome image loaded, the Gray profile with the corresponding TRC will be used
///
#[derive(Builder)]
pub struct IccAssign {}

impl Command for IccAssign {
    fn name() -> &'static str {
        "icc_assign"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// icc_convert_to profile [intent]
/// ```
///
/// Converts the current image to the specified ICC profile.
/// One of the following special arguments may be provided to use the respective built-in profiles: **sRGB**, **sRGBlinear**, **Rec2020**, **Rec2020linear**, **graysrgb**, **grayrec2020**, **graylinear** or **working** to set the working mono or RGB color profile, (for mono images only) **linear**, or the path to an ICC profile file may be provided. If a built-in profile is specified with a monochrome image loaded, the Gray profile with the corresponding TRC will be used.
///
/// A second argument may be provided to specify the color transform intent: this should be one of **perceptual**, **relative** (for relative colorimetric), **saturation** or **absolute** (for absolute colorimetric)
///
#[derive(Builder)]
pub struct IccConvertTo {}

impl Command for IccConvertTo {
    fn name() -> &'static str {
        "icc_convert_to"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// icc_remove
/// ```
///
/// Removes the ICC profile from the current image, if it has one
///
#[derive(Builder)]
pub struct IccRemove {}

impl Command for IccRemove {
    fn name() -> &'static str {
        "icc_remove"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// idiv filename
/// ```
///
/// Divides the loaded image by the image **filename**.
/// Result will be in 32 bits per channel if allowed in the preferences.
///
/// See also FDIV
///
/// Links: :ref:`fdiv <fdiv>`
///
#[derive(Builder)]
pub struct Idiv {}

impl Command for Idiv {
    fn name() -> &'static str {
        "idiv"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// imul filename
/// ```
///
/// Multiplies image **filename** by the loaded image.
/// Result will be in 32 bits per channel if allowed in the preferences
///
#[derive(Builder)]
pub struct Imul {}

impl Command for Imul {
    fn name() -> &'static str {
        "imul"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// invght -D= [-B=] [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels]
/// ```
///
/// Inverts a generalised hyperbolic stretch. It provides the inverse transformation of GHT, if provided with the same parameters, undoes a GHT command, possibly returning to a linear image. It can also work the same way as GHT but for images in negative
///
/// Links: :ref:`ght <ght>`
///
#[derive(Builder)]
pub struct Invght {}

impl Command for Invght {
    fn name() -> &'static str {
        "invght"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// invmodasinh -D= [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels]
/// ```
///
/// Inverts a modified arcsinh stretch. It provides the inverse transformation of MODASINH, if provided with the same parameters, undoes a MODASINH command, possibly returning to a linear image. It can also work the same way as MODASINH but for images in negative
///
/// Links: :ref:`modasinh <modasinh>`
///
#[derive(Builder)]
pub struct Invmodasinh {}

impl Command for Invmodasinh {
    fn name() -> &'static str {
        "invmodasinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// invmtf low mid high [channels]
/// ```
///
/// Inverts a midtones transfer function. It provides the inverse transformation of MTF, if provided with the same parameters, undoes a MTF command, possibly returning to a linear image. It can also work the same way as MTF but for images in negative
///
/// Links: :ref:`mtf <mtf>`
///
#[derive(Builder)]
pub struct Invmtf {}

impl Command for Invmtf {
    fn name() -> &'static str {
        "invmtf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// isub filename
/// ```
///
/// Subtracts the loaded image by the image **filename**.
/// Result will be in 32 bits per channel if allowed in the preferences, so capable of storing negative values. To clip negative value, use 16 bit mode or use the THRESHLO command
///
/// Links: :ref:`threshlo <threshlo>`
///
#[derive(Builder)]
pub struct Isub {}

impl Command for Isub {
    fn name() -> &'static str {
        "isub"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// jsonmetadata FITS_file [-stats_from_loaded] [-nostats] [-out=]
/// ```
///
/// Dumps metadata and statistics of the currently loaded image in JSON form. The file name is required, even if the image is already loaded. Image data may not be read from the file if it is the current loaded image and if the **-stats_from_loaded** option is passed. Statistics can be disabled by providing the **-nostats** option. A file containing the JSON data is created with default file name '$(FITS_file_without_ext).json' and can be changed with the **-out=** option
///
#[derive(Builder)]
pub struct Jsonmetadata {}

impl Command for Jsonmetadata {
    fn name() -> &'static str {
        "jsonmetadata"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// light_curve sequencename channel [-autoring] { -at=x,y | -wcs=ra,dec } { -refat=x,y | -refwcs=ra,dec } ...
/// light_curve sequencename channel [-autoring] -ninastars=file
/// ```
///
/// Analyses several stars with aperture photometry in a sequence of images and produces a light curve for one, calibrated by the others. The first coordinates, in pixels if **-at=** is used or in degrees if **-wcs=** is used, are for the star whose light will be plotted, the others for the comparison stars.
/// Alternatively, a list of target and reference stars can be passed in the format of the NINA exoplanet plugin star list, with the **-ninastars=** option. Siril will verify that all reference stars can be used before actually using them. A data file is created in the current directory named light_curve.dat, Siril plots the result to a PNG image if available
/// The ring radii for the annulus can either be configured in the settings or set to a factor of the reference image's FWHM if **-autoring** is passed. These autoring sizes are 4.2 time and 6.3 times the FWHM for the inner and outer radii, respectively.
///
/// See also the **setphot** command to set the same way the aperture radius size.
///
/// See also SEQPSF for operations on single star
///
/// Links: :ref:`seqpsf <seqpsf>`
///
#[derive(Builder)]
pub struct LightCurve {}

impl Command for LightCurve {
    fn name() -> &'static str {
        "light_curve"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// limit { -clip | -posrescale | -rescale }
/// ```
///
/// Limits pixel values in 32-bit images to the range 0.0 to 1.0. This command does not apply to 16-bit images as there cannot be out-of-range values. Range limiting can be done in one of the following ways:
///
/// **-clip**: this option simply clips all negative pixels to 0.0 and all pixels with a value > 1.0 to 1.0.
/// **-posrescale**: this option scales all positive pixel values so that the maximum value is 1.0, clipping any negative pixels to 0.0. For 3-channel images the same scaling factor is applied to all channels. If the maximum pixel value is already <= 1.0 negative pixels will still be clipped but no scaling factor will be applied to positive pixels.
/// **-rescale**: using this option, if there are any negative pixel values the image will have a constant value added to all pixel values so that the minimum value is 0.0. Then if the maximum pixel value is > 1.0, a scaling factor is applied so that the maximum pixel value is scaled to 1.0.
///
/// Note that if there are one or more extreme outliers (for example as a result of bad pixels) the **-rescale** and **-posrescale** options may produce an unexpected result. This can be mitigated by applying cosmetic correction to the image first
///
#[derive(Builder)]
pub struct Limit {}

impl Command for Limit {
    fn name() -> &'static str {
        "limit"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// linear_match reference low high
/// ```
///
/// Computes and applies a linear function between a **reference** image and the loaded image.
///
/// The algorithm will ignore all reference pixels whose values are outside of the [**low**, **high**] range
///
#[derive(Builder)]
pub struct LinearMatch {}

impl Command for LinearMatch {
    fn name() -> &'static str {
        "linear_match"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// link basename [-date] [-start=index] [-out=]
/// ```
///
/// Same as CONVERT but converts only FITS files found in the current working directory. This is useful to avoid conversions of JPEG results or other files that may end up in the directory. The additional argument **-date** enables sorting files with their DATE-OBS value instead of with their name alphanumerically
///
/// Links: :ref:`convert <convert>`
///
#[derive(Builder)]
pub struct Link {}

impl Command for Link {
    fn name() -> &'static str {
        "link"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// linstretch -BP= [-sat] [-clipmode=] [channels] [-clipmode=]
/// ```
///
/// Stretches the image linearly to a new black point BP.
/// The argument **[channels]** may optionally be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels.
/// Optionally the parameter **-sat** may be used to apply the linear stretch to the image saturation channel. This argument only works if all channels are selected. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Linstretch {}

impl Command for Linstretch {
    fn name() -> &'static str {
        "linstretch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// livestack filename
/// ```
///
/// Process the provided image for live stacking. Only possible after START_LS. The process involves calibrating the incoming file if configured in START_LS, demosaicing if it's an OSC image, registering and stacking. The temporary result will be in the file live_stack_00001.fit until a new option to change it is added
///
/// Links: :ref:`start_ls <start_ls>`
///
/// |
///
/// .. warning::
///
///     Note that the live stacking commands put Siril in a state in which it's not 
///     able to process other commands. After START_LS, only LIVESTACK, STOP_LS and 
///     EXIT can be called until STOP_LS is called to return Siril in its normal, 
///     non-live-stacking, state.
///
#[derive(Builder)]
pub struct Livestack {}

impl Command for Livestack {
    fn name() -> &'static str {
        "livestack"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// load filename[.ext]
/// ```
///
/// Loads the image **filename** from the current working directory, which becomes the 'currently loaded image' used in many of the single-image commands.
/// It first attempts to load **filename**, then **filename**.fit, **filename**.fits and finally all supported formats.
/// This scheme is applicable to every Siril command that involves reading files
///
#[derive(Builder)]
pub struct Load {}

impl Command for Load {
    fn name() -> &'static str {
        "load"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// log
/// ```
///
/// Computes and applies a logarithmic scale to the loaded image, using the following formula: log(1 - (value - min) / (max - min)), with min and max being the minimum and maximum pixel value for the channel
///
#[derive(Builder)]
pub struct Log {}

impl Command for Log {
    fn name() -> &'static str {
        "log"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// makepsf clear
/// makepsf load filename
/// makepsf save [filename]
/// makepsf blind [-l0] [-si] [-multiscale] [-lambda=] [-comp=] [-ks=] [-savepsf=]
/// makepsf stars [-sym] [-ks=] [-savepsf=]
/// makepsf manual { -gaussian | -moffat | -disc | -airy } [-fwhm=] [-angle=] [-ratio=] [-beta=] [-dia=] [-fl=] [-wl=] [-pixelsize=] [-obstruct=] [-ks=] [-savepsf=]
/// ```
///
/// Generates a PSF for use with deconvolution, any of the three methods exposed by RL, SB or WIENER commands. One of the following must be given as the first argument: **clear** (clears the existing PSF), **load** (loads a PSF from a file), **save** (saves the current PSF), **blind** (blind estimate of tke PSF), **stars** (generates a PSF based on measured stars from the image) or **manual** (generates a PSF manually based on a function and parameters).
///
/// No additional arguments are required when using the **clear** argument.
///
/// To load a previously saved PSF the **load** argument requires the PSF *filename* as a second argument. This may be in any format that Siril has been compiled with support for, but it must be square and should ideally be odd.
///
/// To save a previously generated PSF the argument **save** is used. Optionally, a filename may be provided (this must have one of the extensions ".fit", ".fits", ".fts" or ".tif") but if none is provided the PSF will be named based on the name of the open file or sequence.
///
/// For **blind**, the following optional arguments may be provided: **-l0** uses the l0 descent method, **-si** uses the spectral irregularity method, **-multiscale** configures the l0 method to do a multi-scale PSF estimate, **-lambda=** provides the regularization constant.
///
/// For PSF from detected **stars** the only optional parameter is **-sym**, which configures the PSF to be symmetric.
///
/// For a **manual** PSF, one of **-gaussian**, **-moffat**, **-disc** or **-airy** can be provided to specify the PSF function, Gaussian by default. For Gaussian or Moffat PSFs the optional arguments **-fwhm=**, **-angle=** and **-ratio=** may be provided. For Moffat PSFs the optional argument **-beta=** may also be provided. If these values are omitted, they default to the same values as in the deconvolution dialog. For disc PSFs only the argument **-fwhm=** is required, which for this function is used to set the *diameter* of the PSF. For Airy PSFs the following arguments may be provided: **-dia=** (sets the telescope diameter), **-fl=** (sets the telescope focal length), **-wl=** (sets the wavelength to calculate the Airy diffraction pattern for), **-pixelsize=** (sets the sensor pixel size), **-obstruct=** (sets the central obstruction as a percentage of the overall aperture area). If these parameters are not provided, wavelength will default to 525nm and central obstruction will default to 0%. Siril will attempt to read the others from the open image, but some imaging software may not provide all of them in which case you will get bad results, and note the metadata may not be populated for SER format videos. You will learn from experience which are safe to omit for your particular imaging setup.
///
/// For any of the above PSF generation options the optional argument **-ks=** may be provided to set the PSF dimension, and the optional argument **-savepsf=\ filename** may be used to save the generated PSF: a filename must be provided and the same filename extension requirements apply as for **makepsf save filename**
///
/// Links: :ref:`psf <psf>`, :ref:`rl <rl>`, :ref:`sb <sb>`, :ref:`wiener <wiener>`
///
#[derive(Builder)]
pub struct Makepsf {}

impl Command for Makepsf {
    fn name() -> &'static str {
        "makepsf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// merge sequence1 sequence2 [sequence3 ...] output_sequence
/// ```
///
/// Merges several sequences of the same type (FITS images, FITS sequence or SER) and same image properties into a new sequence with base name **newseq** created in the current working directory, with the same type. The input sequences can be in different directories, can specified either in absolute or relative path, with the exact .seq name or with only the base name with or without the trailing '\_'
///
#[derive(Builder)]
pub struct Merge {}

impl Command for Merge {
    fn name() -> &'static str {
        "merge"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// merge_cfa file_CFA0 file_CFA1 file_CFA2 file_CFA3 bayerpattern
/// ```
///
/// Builds a Bayer masked color image from 4 separate images containing the data from Bayer subchannels CFA0, CFA1, CFA2 and CFA3. (The corresponding command to split the CFA pattern into subchannels is **split_cfa**.) This function can be used as part of a workflow applying some processing to the individual Bayer subchannels prior to demosaicing. The fifth parameter **bayerpattern** specifies the Bayer matrix pattern to recreate: **bayerpattern** should be one of 'RGGB', 'BGGR', 'GRBG' or 'GBRG'
///
#[derive(Builder)]
pub struct MergeCfa {}

impl Command for MergeCfa {
    fn name() -> &'static str {
        "merge_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// mirrorx [-bottomup]
/// ```
///
/// Flips the loaded image about the horizontal axis. Option **-bottomup** will only flip it if it's not already bottom-up
///
#[derive(Builder)]
pub struct Mirrorx {}

impl Command for Mirrorx {
    fn name() -> &'static str {
        "mirrorx"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// mirrorx_single image
/// ```
///
/// Flips the image about the horizontal axis, only if needed (if it's not already bottom-up). It takes the image file name as argument, allowing it to avoid reading image data entirely if no flip is required. Image is overwritten if a flip is made
///
#[derive(Builder)]
pub struct MirrorxSingle {}

impl Command for MirrorxSingle {
    fn name() -> &'static str {
        "mirrorx_single"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// mirrory
/// ```
///
/// Flips the image about the vertical axis
///
#[derive(Builder)]
pub struct Mirrory {}

impl Command for Mirrory {
    fn name() -> &'static str {
        "mirrory"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// modasinh -D= [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels]
/// ```
///
/// Modified arcsinh stretch based on the work of the ghsastro.co.uk team.
///
/// The argument **-D=** defines the strength of the stretch, between 0 and 10. This is the only mandatory argument. The following optional arguments further tailor the stretch:
/// **LP** defines a shadow preserving range between 0 and SP where the stretch will be linear, preserving shadow detail;
/// **SP** defines the symmetry point of the stretch, between 0 and 1, which is the point at which the stretch will be most intense;
/// **HP** defines a region between HP and 1 where the stretch is linear, preserving highlight details and preventing star bloat.
/// If omitted LP and SP default to 0.0 ad HP defaults to 1.0.
/// An optional argument (either **-human**, **-even** or **-independent**) can be passed to select either human-weighted or even-weighted luminance or independent colour channels for colour stretches. The argument is ignored for mono images. Alternatively, the argument **-sat** specifies that the stretch is performed on image saturation - the image must be color and all channels must be selected for this to work.
/// Optionally the parameter **[channels]** may be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Modasinh {}

impl Command for Modasinh {
    fn name() -> &'static str {
        "modasinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// mtf low mid high [channels]
/// ```
///
/// Applies midtones transfer function to the current loaded image.
///
/// Three parameters are needed, **low**, **midtones** and **high** where midtones balance parameter defines a nonlinear histogram stretch in the [0,1] range. For an automatic determination of the parameters, see AUTOSTRETCH.
/// Optionally the parameter **[channels]** may be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels
///
/// Links: :ref:`autostretch <autostretch>`
///
#[derive(Builder)]
pub struct Mtf {}

impl Command for Mtf {
    fn name() -> &'static str {
        "mtf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// neg
/// ```
///
/// Changes pixel values of the currently loaded image to a negative view, like 1-value for 32 bits, 65535-value for 16 bits. This does not change the display mode
///
#[derive(Builder)]
pub struct Neg {}

impl Command for Neg {
    fn name() -> &'static str {
        "neg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// nozero level
/// ```
///
/// Replaces null values by **level** values. Useful before an idiv or fdiv operation, mostly for 16-bit images
///
#[derive(Builder)]
pub struct Nozero {}

impl Command for Nozero {
    fn name() -> &'static str {
        "nozero"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// offline
/// ```
///
/// Sets Siril to offline mode. In this mode networking functions such as remote catalogue lookups, update of git repositories etc. are unavailable. Cached data is still accessible
///
#[derive(Builder)]
pub struct Offline {}

impl Command for Offline {
    fn name() -> &'static str {
        "offline"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// offset value
/// ```
///
/// Adds the constant **value** (specified in ADU) to the current image. This constant can take a negative value.
///
/// In 16-bit mode, values of pixels that fall outside of [0, 65535] are clipped. In 32-bit mode, no clipping occurs
///
#[derive(Builder)]
pub struct Offset {}

impl Command for Offset {
    fn name() -> &'static str {
        "offset"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// online
/// ```
///
/// Sets Siril to online mode. In this mode networking functions such as remote catalogue lookups, update of git repositories etc. is allowed
///
#[derive(Builder)]
pub struct Online {}

impl Command for Online {
    fn name() -> &'static str {
        "online"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// parse str [-r]
/// ```
///
/// Parses the string **str** using the information contained in the header of the image currently loaded. Main purpose of this command is to debug path parsing of header keys which can be used in other commands.
/// Option **-r** specifies the string is to be interpreted in read mode. In read mode, all wildcards defined in string **str** are used to find a file name matching the pattern. Otherwise, default mode is write mode and wildcards, if any, are removed from the string to be parsed.
///
/// If **str** starts with *$def* prefix, it will be recognized as a reserved keyword and looked for in the strings stored in gui_prepro.dark_lib, gui_prepro.flat_lib, gui_prepro.bias_lib or gui_prepro.stack_default for *$defdark*, *$defflat*, *$defbias* or *$defstack* respectively.
/// The keyword *$seqname$* can also be used when a sequence is loaded
///
#[derive(Builder)]
pub struct Parse {}

impl Command for Parse {
    fn name() -> &'static str {
        "parse"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// pcc [-limitmag=[+-]] [-catalog=] [-bgtol=lower,upper]
/// ```
///
/// Run the Photometric Color Correction on the loaded plate-solved image.
///
/// The limit magnitude of stars is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The star catalog used is NOMAD by default, it can be changed by providing **-catalog=apass**, **-catalog=localgaia** or **-catalog=gaia**. If installed locally, the remote NOMAD (the complete version) can be forced by providing **-catalog=nomad**
/// Background reference outlier tolerance can be specified in sigma units using **-bgtol=lower,upper**: these default to -2.8 and +2.0
///
#[derive(Builder)]
pub struct Pcc {}

impl Command for Pcc {
    fn name() -> &'static str {
        "pcc"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// platesolve [-force] [image_center_coords] [-focal=] [-pixelsize=]
/// platesolve ... [-noflip] [-downscale] [-order=] [-radius=] [-disto=]
/// platesolve ... [-limitmag=[+-]] [-catalog=] [-nocrop]
/// platesolve ... [-localasnet [-blindpos] [-blindres]]
/// ```
///
/// Plate solve the loaded image.
/// If the image has already been plate solved nothing will be done, unless the **-force** argument is passed to force a new solve. If WCS or other image metadata is erroneous or missing, arguments must be passed:
/// the approximate image center coordinates can be provided in decimal degrees or degree/hour minute second values (J2000 with colon separators), with right ascension and declination values separated by a comma or a space (not mandatory for astrometry.net).
/// focal length and pixel size can be passed with **-focal=** (in mm) and **-pixelsize=** (in microns), overriding values from image and settings. See also options to solve blindly with local Astrometry.net
///
/// Unless **-noflip** is specified, if the image is detected as being upside-down, it will be flipped.
/// For faster star detection in big images, downsampling the image is possible with **-downscale**.
/// The solve can account for distortions using SIP convention with polynomials up to order 5. Default value is taken form the astrometry preferences. This can be changed with the option **-order=** giving a value between 1 and 5.
/// When using Siril solver local catalogues or with local Astrometry.net, if the initial solve is not successful, the solver will search for a solution within a cone of radius specified with **-radius=** option. If no value is passed, the search radius is taken from the astrometry preferences. Siril near search can be disabled by passing a value of 0. (cannot be disabled for Astrometry.net).
/// You can save the current solution as a distortion file with the option **-disto=**.
///
/// Images can be either plate solved by Siril using a star catalog and the global registration algorithm or by astrometry.net's local solve-field command (enabled with **-localasnet**).
///
/// **Siril platesolver options:**
/// The limit magnitude of stars used for plate solving is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The choice of the star catalog is automatic unless the **-catalog=** option is passed: if local catalogs are installed, they are used, otherwise the choice is based on the field of view and limit magnitude. If the option is passed, it forces the use of the catalog given in argument, with possible values: tycho2, nomad, localgaia, gaia, ppmxl, brightstars, apass.
/// If the computed field of view is larger than 5 degrees, star detection will be bounded to a cropped area around the center of the image unless **-nocrop** option is passed.
///
/// **Astrometry.net solver options:**
/// Passing options **-blindpos** and/or **-blindres** enables to solve blindly for position and for resolution respectively. You can use these when solving an image with a completely unknown location and sampling
///
#[derive(Builder)]
pub struct Platesolve {}

impl Command for Platesolve {
    fn name() -> &'static str {
        "platesolve"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// pm "expression" [-rescale [low] [high]] [-nosum]
/// ```
///
/// This command evaluates the expression given in argument as in PixelMath tool. The full expression must be between double quotes and variables (that are image names, without extension, located in the working directory in that case) must be surrounded by the token $, e.g. "$image1$ \* 0.5 + $image2$ \* 0.5". A maximum of 10 images can be used in the expression.
/// Image can be rescaled with the option **-rescale** followed by **low** and **high** values in the range [0, 1]. If no low and high values are provided, default values are set to 0 and 1. Another optional argument, **-nosum** tells Siril not to sum exposure times. This impacts FITS keywords such as LIVETIME and STACKCNT
///
#[derive(Builder)]
pub struct Pm {}

impl Command for Pm {
    fn name() -> &'static str {
        "pm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// profile -from=x,y -to=x,y [-tri] [-cfa] [-arcsec] { [-savedat] | [-filename=] } [-layer=] [-width=] [-spacing=] ["-title=My Plot"]
/// ```
///
/// Generates an intensity profile plot between 2 points in the image, also known as a *cut*. The arguments may be provided in any order. The arguments **-to=x,y** and **-from=x,y** are mandatory.
///
/// The argument **-layer=\ {red \| green \| blue \| lum \| col}** specifies which channel (or luminance or colour) to plot if the image is color. It may also be used with the **-tri** option, which generates 3 parallel equispaced profiles each separated by **-spacing=** pixels, but note that for tri profiles the **col** option will be treated the same as **lum**.
///
/// The option **-cfa** selects CFA mode, which generates 4 profiles: 1 for each CFA channel in a Bayer patterned image. This option cannot be used with color images or mono images with no Bayer pattern, and cannot be used at the same time as the **-tri** option.
///
/// The option **-arcsec** causes the x axis to display distance in arcsec, if the necessary metadata is available. If not provided or if metadata is not available, distance will be shown in pixel units.
///
/// The argument **-savedat** will cause the data files to be saved: the filename will be written to the log. Alternatively the argument **-filename=** can be used to specify a filename to write the data file to. (The **-filename=** option implies **-savedat**.)
///
/// The argument **"-title=\ My Title"** sets a custom title "My Title"
///
#[derive(Builder)]
pub struct Profile {}

impl Command for Profile {
    fn name() -> &'static str {
        "profile"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// psf [channel]
/// ```
///
/// Performs a PSF (Point Spread Function) on the selected star and display the results. For headless operation, the selection can be given in pixels using BOXSELECT. If provided, the **channel** argument selects the image channel on which the star will be analyzed. It can be omitted for monochrome images or when run from the GUI with one of the channels active in the view
///
/// Links: :ref:`boxselect <boxselect>`
///
#[derive(Builder)]
pub struct Psf {}

impl Command for Psf {
    fn name() -> &'static str {
        "psf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// pwd
/// ```
///
/// Prints the current working directory
///
#[derive(Builder)]
pub struct Pwd {}

impl Command for Pwd {
    fn name() -> &'static str {
        "pwd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// pyscript scriptname.py [script_argv]
/// ```
///
/// Executes a Siril python script
///
/// The script name must be provided as the first argument. If it is not found in the current working directory, the user-defined script paths specified in Preferences and the local siril-scripts repository will be searched. All subsequent arguments will be treated as script arguments and passed to the script as its argument vector. Note that the specific script must incorporate support for reading input from the argument vector
///
#[derive(Builder)]
pub struct Pyscript {}

impl Command for Pyscript {
    fn name() -> &'static str {
        "pyscript"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// register sequencename [-2pass] [-selected] [-prefix=] [-scale=]
/// register sequencename ... [-layer=] [-transf=] [-minpairs=] [-maxstars=] [-nostarlist] [-disto=]
/// register sequencename ... [-interp=] [-noclamp]
/// register sequencename ... [-drizzle [-pixfrac=] [-kernel=] [-flat=]]
/// ```
///
/// Finds and optionally performs geometric transforms on images of the sequence given in argument so that they may be superimposed on the reference image. Using stars for registration, this algorithm only works with deep sky images. Star detection options can be changed using **SETFINDSTAR** or the *Dynamic PSF* dialog.
///
/// All images of the sequence will be registered unless the option **-selected** is passed, in that case the excluded images will not be processed.
/// The **-2pass** option will only compute the transforms but not generate the transformed images, **-2pass** adds a preliminary pass to the algorithm to find a good reference image before computing the transforms, based on image quality and framing. To generate transformed images after this pass, use SEQAPPLYREG.
/// If created, the output sequence name will start with the prefix "r\_" unless otherwise specified with **-prefix=** option. The output images can be rescaled by passing a **-scale=** argument with a float value between 0.1 and 3.
///
/// **Image transformation options:**
///
/// The detection is done on the green layer for colour images, unless specified by the **-layer=** option with an argument ranging from 0 to 2 for red to blue.
/// **-transf=** specifies the use of either **shift**, **similarity**, **affine** or **homography** (default) transformations respectively.
/// **-minpairs=** will specify the minimum number of star pairs a frame must have with the reference frame, otherwise the frame will be dropped and excluded from the sequence.
/// **-maxstars=** will specify the maximum number of stars to find within each frame (must be between 100 and 2000). With more stars, a more accurate registration can be computed, but will take more time to run.
/// **-nostarlist** disables saving the star lists to disk.
/// **-disto=** uses distortion terms from a previous platesolve solution (with a SIP order > 1). It takes as parameter either **image** to use the solution contained in the currently loaded image, **file** followed by the path to the image containing the solution or **master** to load automatically the matching distortion master corresponding to each image. When using this option, the polynomials are used both to correct star positions before computing the transformation and to undistort the images when output images are exported.
///
/// **Image interpolation options:**
///
/// By default, transformations are applied to register the images by using interpolation.
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}. If **none** is passed, the transformation is forced to shift and a pixel-wise shift is applied to each image without any interpolation.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument.
///
/// **Image drizzle options:**
///
/// Otherwise, the images can be exported using HST drizzle algorithm by passing the argument **-drizzle** which can take the additional options:
/// **-pixfrac=** sets the pixel fraction (default = 1.0).
/// The **-kernel=** argument sets the drizzle kernel and must be followed by one of **point**, **turbo**, **square**, **gaussian**, **lanczos2** or **lanczos3**. The default is **square**.
/// The **-flat=** argument specifies a master flat to weight the drizzled input pixels (default is no flat).
///
/// Note: when using **-drizzle** on images taken with a color camera, the input images must not be debayered. In that case, star detection will always occur on the green pixels
///
/// Links: :ref:`setfindstar <setfindstar>`, :ref:`psf <psf>`, :ref:`seqapplyreg <seqapplyreg>`
///
#[derive(Builder)]
pub struct Register {}

impl Command for Register {
    fn name() -> &'static str {
        "register"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// requires min_version [obsolete_version]
/// ```
///
/// Returns an error if the version of Siril is older than the minimum required version passed in the first argument. Optionally, takes a second argument for the Siril version at which the script is obsolete: returns an error if the version of Siril is **newer than or equal to** the one passed in the second argument.
///
/// Example: *requires 1.2.0 1.4.0* allows the script to run for all of the 1.2.x series and 1.3.x series, but will not run for any versions earlier than 1.2.0 or for version 1.4.0 or any later versions
///
#[derive(Builder)]
pub struct Requires {}

impl Command for Requires {
    fn name() -> &'static str {
        "requires"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// resample { factor | -width= | -height= | -maxdim= } [-interp=] [-noclamp]
/// ```
///
/// Resamples the loaded image, either with a factor **factor** or for the target width or height provided by either of **-width=**, **-height=** or **-maxdim=**. This is generally used to resize images: a factor of 0.5 divides size by 2. The **-maxdim** argument can be used to resize the longest dimension of the image to a set size, which can be useful for optimizing images for certain websites, e.g. social media websites.
/// In the graphical user interface, we can see that several interpolation algorithms are proposed.
///
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument
///
#[derive(Builder)]
pub struct Resample {}

impl Command for Resample {
    fn name() -> &'static str {
        "resample"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// rgbcomp red green blue [-out=result_filename] [-nosum]
/// rgbcomp -lum=image { rgb_image | red green blue } [-out=result_filename] [-nosum]
/// ```
///
/// Creates an RGB composition using three independent images, or an LRGB composition using the optional luminance image and three monochrome images or a color image. Result image is called composed_rgb.fit or composed_lrgb.fit unless another name is provided in the optional argument. Another optional argument, **-nosum** tells Siril not to sum exposure times. This impacts FITS keywords such as LIVETIME and STACKCNT
///
#[derive(Builder)]
pub struct Rgbcomp {}

impl Command for Rgbcomp {
    fn name() -> &'static str {
        "rgbcomp"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// rgradient xc yc dR dalpha
/// ```
///
/// Creates two images, with a radial shift (**dR** in pixels) and a rotational shift (**dalpha** in degrees) with respect to the point (**xc**, **yc**).
///
/// Between these two images, the shifts have the same amplitude, but an opposite sign. The two images are then added to create the final image. This process is also called Larson Sekanina filter
///
#[derive(Builder)]
pub struct Rgradient {}

impl Command for Rgradient {
    fn name() -> &'static str {
        "rgradient"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// rl [-loadpsf=] [-alpha=] [-iters=] [-stop=] [-gdstep=] [-tv] [-fh] [-mul]
/// ```
///
/// Restores an image using the Richardson-Lucy method.
///
/// Optionally, a PSF may be loaded using the argument **-loadpsf=\ filename** (created with MAKEPSF).
///
/// The number of iterations is provide by **-iters** (the default is 10).
///
/// The type of regularization can be set with **-tv** for Total Variation, or **-fh** for the Frobenius norm of the Hessian matrix (the default is none) and **-alpha=** provides the regularization strength (lower value = more regularization, default = 3000).
///
/// By default the gradient descent method is used with a default step size of 0.0005, however the multiplicative method may be specified with **-mul**.
///
/// The stopping criterion may be activated by specifying a stopping limit with **-stop=**
///
/// Links: :ref:`psf <psf>`, :ref:`makepsf <makepsf>`
///
#[derive(Builder)]
pub struct Rl {}

impl Command for Rl {
    fn name() -> &'static str {
        "rl"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// rmgreen [-nopreserve] [type] [amount]
/// ```
///
/// Applies a chromatic noise reduction filter. It removes green tint in the current image. This filter is based on PixInsight's SCNR and it is also the same filter used by HLVG plugin in Photoshop.
/// Lightness is preserved by default but this can be disabled with the **-nopreserve** switch.
///
/// **Type** can take values 0 for average neutral, 1 for maximum neutral, 2 for maximum mask, 3 for additive mask, defaulting to 0. The last two can take an **amount** argument, a value between 0 and 1, defaulting to 1
///
#[derive(Builder)]
pub struct Rmgreen {}

impl Command for Rmgreen {
    fn name() -> &'static str {
        "rmgreen"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// rotate degree [-nocrop] [-interp=] [-noclamp]
/// ```
///
/// Rotates the loaded image by an angle of **degree** value. The option **-nocrop** can be added to avoid cropping to the image size (black borders will be added).
///
/// Note: if a selection is active, i.e. by using a command \`boxselect\` before \`rotate\`, the resulting image will be a rotated crop. In this particular case, the option **-nocrop** will be ignored if passed.
///
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}. If **none** is passed, the transformation is forced to shift and a pixel-wise shift is applied to each image without any interpolation.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument
///
#[derive(Builder)]
pub struct Rotate {}

impl Command for Rotate {
    fn name() -> &'static str {
        "rotate"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// rotatePi
/// ```
///
/// Rotates the loaded image of an angle of 180° around its center. This is equivalent to the command "ROTATE 180" or "ROTATE -180"
///
/// Links: :ref:`rotate <rotate>`
///
#[derive(Builder)]
pub struct RotatePi {}

impl Command for RotatePi {
    fn name() -> &'static str {
        "rotatePi"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// satu amount [background_factor [hue_range_index]]
/// ```
///
/// Enhances the color saturation of the loaded image. Try iteratively to obtain best results.
/// **amount** can be a positive number to increase color saturation, negative to decrease it, 0 would do nothing, 1 would increase it by 100%
/// **background_factor** is a factor to (median + sigma) used to set a threshold for which only pixels above it would be modified. This allows background noise to not be color saturated, if chosen carefully. Defaults to 1. Setting 0 disables the threshold.
/// **hue_range_index** can be [0, 6], meaning: 0 for pink to orange, 1 for orange to yellow, 2 for yellow to cyan, 3 for cyan, 4 for cyan to magenta, 5 for magenta to pink, 6 for all (default)
///
#[derive(Builder)]
pub struct Satu {}

impl Command for Satu {
    fn name() -> &'static str {
        "satu"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// save filename [-chksum]
/// ```
///
/// Saves current image to **filename**.fit (or .fits, depending on your preferences, see SETEXT) in the current working directory. The image remains loaded. **filename** can contain a path as long as the directory already exists. The **-chksum** option stores checksum keywords (CHECKSUM and DATASUM) in the FITS header
///
/// Links: :ref:`setext <setext>`
///
#[derive(Builder)]
pub struct Save {}

impl Command for Save {
    fn name() -> &'static str {
        "save"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// savebmp filename
/// ```
///
/// Saves current image under the form of a bitmap file with 8-bit per channel: **filename**.bmp (BMP 24-bit)
///
#[derive(Builder)]
pub struct Savebmp {}

impl Command for Savebmp {
    fn name() -> &'static str {
        "savebmp"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// savejpg filename [quality]
/// ```
///
/// Saves current image into a JPG file: **filename**.jpg.
///
/// The compression quality can be adjusted using the optional **quality** value, 100 being the best and default, while a lower value increases the compression ratio
///
#[derive(Builder)]
pub struct Savejpg {}

impl Command for Savejpg {
    fn name() -> &'static str {
        "savejpg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// savejxl filename [-effort=] [-quality=] [-8bit]
/// ```
///
/// Saves current image into a JPG XL file: **filename**.jxl.
///
/// All other arguments are optional. The quality setting expresses a maximum permissible distance between the original and the compressed image: the **-quality=** argument may be provided and must be specified as a floating point number between 0.0 and 10.0. A higher quality means better quality, but larger file size. Quality = 10.0 is mathematically lossless, quality = 9.0 is visually lossless and quality = 0 is visually poor but gives very small file sizes. The default value is 9.0; typical values range from 7.0 to 10.0. The compression effort can be adjusted using the optional **-effort=** value, 9 being the most effort but very slow, while a lower value increases the compression ratio. Values above 7 are not recommended as they can be very slow and produce little if any benefit to file size, in fact sometimes effort = 9 can produce larger files. If this argument is omitted the default value of 7 is used. An option **-8bit** may be provided to force output to be 8 bits per pixel
///
#[derive(Builder)]
pub struct Savejxl {}

impl Command for Savejxl {
    fn name() -> &'static str {
        "savejxl"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// savepng filename
/// ```
///
/// Saves current image into a PNG file: **filename**.png, with 16 bits per channel if the loaded image is 16 or 32 bits, and 8 bits per channel if the loaded image is 8 bits
///
#[derive(Builder)]
pub struct Savepng {}

impl Command for Savepng {
    fn name() -> &'static str {
        "savepng"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// savepnm filename
/// ```
///
/// Saves current image under the form of a NetPBM file format with 16-bit per channel.
///
/// The extension of the output will be **filename**.ppm for RGB image and **filename**.pgm for gray-level image
///
#[derive(Builder)]
pub struct Savepnm {}

impl Command for Savepnm {
    fn name() -> &'static str {
        "savepnm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// savetif filename [-astro] [-deflate]
/// ```
///
/// Saves current image under the form of a uncompressed TIFF file with 16-bit per channel: **filename**.tif. The option **-astro** allows saving in Astro-TIFF format, while **-deflate** enables compression.
///
/// See also SAVETIF32 and SAVETIF8
///
#[derive(Builder)]
pub struct Savetif {}

impl Command for Savetif {
    fn name() -> &'static str {
        "savetif"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// savetif32 filename [-astro] [-deflate]
/// ```
///
/// Same command as SAVETIF but the output file is saved in 32-bit per channel: **filename**.tif. The option **-astro** allows saving in Astro-TIFF format, while **-deflate** enables compression
///
/// Links: :ref:`savetif <savetif>`
///
#[derive(Builder)]
pub struct Savetif32 {}

impl Command for Savetif32 {
    fn name() -> &'static str {
        "savetif32"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// savetif8 filename [-astro] [-deflate]
/// ```
///
/// Same command as SAVETIF but the output file is saved in 8-bit per channel: **filename**.tif. The option **-astro** allows saving in Astro-TIFF format, while **-deflate** enables compression
///
/// Links: :ref:`savetif <savetif>`
///
#[derive(Builder)]
pub struct Savetif8 {}

impl Command for Savetif8 {
    fn name() -> &'static str {
        "savetif8"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// sb [-loadpsf=] [-alpha=] [-iters=]
/// ```
///
/// Restores an image using the Split Bregman method.
///
/// Optionally, a PSF may be loaded using the argument **-loadpsf=\ filename**.
///
/// The number of iterations is provide by **-iters** (the default is 1).
///
/// The regularization factor **-alpha=** provides the regularization strength (lower value = more regularization, default = 3000)
///
/// Links: :ref:`psf <psf>`
///
#[derive(Builder)]
pub struct Sb {}

impl Command for Sb {
    fn name() -> &'static str {
        "sb"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// select sequencename from to
/// ```
///
/// This command allows easy mass selection of images in the sequence **sequencename** (from **from** to **to** included). This is a selection for later processing.
/// See also UNSELECT
///
/// Links: :ref:`unselect <unselect>`
///
/// |
/// Examples:
///
/// `select . 0 0`
/// selects the first of the currently loaded sequence
///
/// `select sequencename 1000 1200`
/// selects 201 images starting from number 1000 in sequence named sequencename
///
/// The second number can be greater than the number of images to just go up to the end.
///
#[derive(Builder)]
pub struct Select {}

impl Command for Select {
    fn name() -> &'static str {
        "select"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqapplyreg sequencename [-prefix=] [-scale=] [-layer=] [-framing=]
/// seqapplyreg sequencename ... [-interp=] [-noclamp]
/// seqapplyreg sequencename ... [-drizzle [-pixfrac=] [-kernel=] [-flat=]]
/// seqapplyreg sequencename ... [-filter-fwhm=value[%|k]] [-filter-wfwhm=value[%|k]] [-filter-round=value[%|k]] [-filter-bkg=value[%|k]] [-filter-nbstars=value[%|k]] [-filter-quality=value[%|k]] [-filter-incl[uded]]
/// ```
///
/// Applies geometric transforms on images of the sequence given in argument so that they may be superimposed on the reference image, using registration data previously computed (see REGISTER).
/// The output sequence name starts with the prefix **"r\_"** unless otherwise specified with **-prefix=** option.
/// The registration is done on the first layer for which data exists for RGB images unless specified by **-layer=** option (0, 1 or 2 for R, G and B respectively).
/// The output images can be rescaled by passing a **-scale=** argument with a float value between 0.1 and 3.
///
/// Automatic framing of the output sequence can be specified using **-framing=** keyword followed by one of the methods in the list { current \| min \| max \| cog } :
/// **-framing=max** (bounding box) will project each image and compute its shift wrt. reference image. The resulting sequence can then be stacked using option **-maximize** of STACK command which will create the full image encompassing all images of the sequence.
/// **-framing=min** (common area) crops each image to the area it has in common with all images of the sequence.
/// **-framing=cog** determines the best framing position as the center of gravity (cog) of all the images.
///
/// **Image interpolation options:**
/// By default, transformations are applied to register the images by using interpolation.
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}. If **none** is passed, the transformation is forced to shift and a pixel-wise shift is applied to each image without any interpolation.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument.
///
/// **Image drizzle options:**
/// Otherwise, the images can be exported using HST drizzle algorithm by passing the argument **-drizzle** which can take the additional options:
/// **-pixfrac=** sets the pixel fraction (default = 1.0).
/// The **-kernel=** argument sets the drizzle kernel and must be followed by one of **point**, **turbo**, **square**, **gaussian**, **lanczos2** or **lanczos3**. The default is **square**.
/// The **-flat=** argument specifies a master flat to weight the drizzled input pixels (default is no flat).
///
/// **Filtering out images:**
/// Images to be registered can be selected based on some filters, like those selected or with best FWHM, with some of the **-filter-\*** options.
///
///
/// Links: :ref:`register <register>`, :ref:`stack <stack>`
///
/// With filtering being some of these in no particular order or number:
///
/// ```text
/// [-filter-fwhm=value[%|k]] [-filter-wfwhm=value[%|k]] [-filter-round=value[%|k]] [-filter-bkg=value[%|k]]
/// [-filter-nbstars=value[%|k]] [-filter-quality=value[%|k]] [-filter-incl[uded]]
/// ```
/// Best images from the sequence can be stacked by using the filtering arguments. Each of these arguments can remove bad images based on a property their name contains, taken from the registration data, with either of the three types of argument values:
/// - a numeric value for the worse image to keep depending on the type of data used (between 0 and 1 for roundness and quality, absolute values otherwise),
/// - a percentage of best images to keep if the number is followed by a % sign,
/// - or a k value for the k.sigma of the worse image to keep if the number is followed by a k sign.
/// It is also possible to use manually selected images, either previously from the GUI or with the select or unselect commands, using the **-filter-included** argument.
///
#[derive(Builder)]
pub struct Seqapplyreg {}

impl Command for Seqapplyreg {
    fn name() -> &'static str {
        "seqapplyreg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqccm sequencename [-prefix=]
/// ```
///
/// Same command as CCM but for the the sequence **sequencename**. Only selected images in the sequence are processed.
///
/// The output sequence name starts with the prefix "ccm" unless otherwise specified with option **-prefix=**
///
/// Links: :ref:`ccm <ccm>`
///
#[derive(Builder)]
pub struct Seqccm {}

impl Command for Seqccm {
    fn name() -> &'static str {
        "seqccm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqclean sequencename [-reg] [-stat] [-sel]
/// ```
///
/// This command clears selection, registration and/or statistics data stored for the sequence **sequencename**.
///
/// You can specify to clear only registration, statistics and/or selection with **-reg**, **-stat** and **-sel** options respectively. All are cleared if no option is passed
///
#[derive(Builder)]
pub struct Seqclean {}

impl Command for Seqclean {
    fn name() -> &'static str {
        "seqclean"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqcosme sequencename [filename].lst [-prefix=]
/// ```
///
/// Same command as COSME but for the the sequence **sequencename**. Only selected images in the sequence are processed.
///
/// The output sequence name starts with the prefix "cosme\_" unless otherwise specified with option **-prefix=**
///
/// Links: :ref:`cosme <cosme>`
///
#[derive(Builder)]
pub struct Seqcosme {}

impl Command for Seqcosme {
    fn name() -> &'static str {
        "seqcosme"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqcosme_cfa sequencename [filename].lst [-prefix=]
/// ```
///
/// Same command as COSME_CFA but for the the sequence **sequencename**. Only selected images in the sequence are processed.
///
/// The output sequence name starts with the prefix "cosme\_" unless otherwise specified with option **-prefix=**
///
/// Links: :ref:`cosme_cfa <cosme_cfa>`
///
#[derive(Builder)]
pub struct SeqcosmeCfa {}

impl Command for SeqcosmeCfa {
    fn name() -> &'static str {
        "seqcosme_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqcrop sequencename x y width height [-prefix=]
/// ```
///
/// Crops the sequence given in argument **sequencename**. Only selected images in the sequence are processed.
///
/// The crop selection is specified by the upper left corner position **x** and **y** and the selection **width** and **height**, like for CROP.
/// The output sequence name starts with the prefix "cropped\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`crop <crop>`
///
#[derive(Builder)]
pub struct Seqcrop {}

impl Command for Seqcrop {
    fn name() -> &'static str {
        "seqcrop"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqextract_Green sequencename [-prefix=]
/// ```
///
/// Same command as EXTRACT_GREEN but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "Green\_" unless otherwise specified with option **-prefix=**
///
#[derive(Builder)]
pub struct SeqextractGreen {}

impl Command for SeqextractGreen {
    fn name() -> &'static str {
        "seqextract_Green"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqextract_Ha sequencename [-prefix=] [-upscale]
/// ```
///
/// Same command as EXTRACT_HA but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "Ha\_" unless otherwise specified with option **-prefix=**
///
#[derive(Builder)]
pub struct SeqextractHa {}

impl Command for SeqextractHa {
    fn name() -> &'static str {
        "seqextract_Ha"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqextract_HaOIII sequencename [-resample=]
/// ```
///
/// Same command as EXTRACT_HAOIII but for the sequence **sequencename**.
///
/// The output sequences names start with the prefixes "Ha\_" and "OIII\_"
///
#[derive(Builder)]
pub struct SeqextractHaOIII {}

impl Command for SeqextractHaOIII {
    fn name() -> &'static str {
        "seqextract_HaOIII"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqfind_cosme sequencename cold_sigma hot_sigma [-prefix=]
/// ```
///
/// Same command as FIND_COSME but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "cc\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`find_cosme <find_cosme>`
///
#[derive(Builder)]
pub struct SeqfindCosme {}

impl Command for SeqfindCosme {
    fn name() -> &'static str {
        "seqfind_cosme"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqfind_cosme_cfa sequencename cold_sigma hot_sigma [-prefix=]
/// ```
///
/// Same command as FIND_COSME_CFA but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "cc\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`find_cosme_cfa <find_cosme_cfa>`
///
#[derive(Builder)]
pub struct SeqfindCosmeCfa {}

impl Command for SeqfindCosmeCfa {
    fn name() -> &'static str {
        "seqfind_cosme_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqfindstar sequencename [-layer=] [-maxstars=]
/// ```
///
/// Same command as FINDSTAR but for the sequence **sequencename**.
///
/// The option **-out=** is not available for this process as all the star list files are saved with the default name *seqname_seqnb.lst*
///
/// Links: :ref:`findstar <findstar>`
///
#[derive(Builder)]
pub struct Seqfindstar {}

impl Command for Seqfindstar {
    fn name() -> &'static str {
        "seqfindstar"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqfixbanding sequencename amount sigma [-prefix=] [-vertical]
/// ```
///
/// Same command as FIXBANDING but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "unband\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`fixbanding <fixbanding>`
///
#[derive(Builder)]
pub struct Seqfixbanding {}

impl Command for Seqfixbanding {
    fn name() -> &'static str {
        "seqfixbanding"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqght sequence -D= [-B=] [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels] [-prefix=]
/// ```
///
/// Same command as GHT but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`ght <ght>`
///
#[derive(Builder)]
pub struct Seqght {}

impl Command for Seqght {
    fn name() -> &'static str {
        "seqght"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqheader sequencename keyword [keyword2 ...] [-sel] [-out=file.csv]
/// ```
///
/// Prints the FITS header value corresponding to the given keys for all images in the sequence. You can write several keys in a row, separated by a space. The **-out=** option, followed by a file name, allows you to print the output in a csv file. The **-sel** option limits the output to the images selected in the sequence
///
#[derive(Builder)]
pub struct Seqheader {}

impl Command for Seqheader {
    fn name() -> &'static str {
        "seqheader"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqinvght sequence -D= [-B=] [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels] [-prefix=]
/// ```
///
/// Same command as INVGHT but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`invght <invght>`
///
#[derive(Builder)]
pub struct Seqinvght {}

impl Command for Seqinvght {
    fn name() -> &'static str {
        "seqinvght"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqinvmodasinh sequence -D= [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels] [-prefix=]
/// ```
///
/// Same command as INVMODASINH but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`invmodasinh <invmodasinh>`
///
#[derive(Builder)]
pub struct Seqinvmodasinh {}

impl Command for Seqinvmodasinh {
    fn name() -> &'static str {
        "seqinvmodasinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqlinstretch sequence -BP= [channels] [-sat] [-prefix=]
/// ```
///
/// Same command as LINSTRETCH but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`linstretch <linstretch>`
///
#[derive(Builder)]
pub struct Seqlinstretch {}

impl Command for Seqlinstretch {
    fn name() -> &'static str {
        "seqlinstretch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqmerge_cfa sequencename0 sequencename1 sequencename2 sequencename3 bayerpattern [-prefixout=]
/// ```
///
/// Merges 4 sequences of images to recombine the Bayer pattern. The sequences are specified in the arguments **sequencename0**, **sequencename1**, **sequencename2** and **sequencename3**.
///
/// The Bayer pattern to be reconstructed must be provided as the second argument as one of RGGB, BGGR, GBRG or GRBG (the order of the Bayer channels must match the order of the specified sequences).
///
/// Note: all 4 input sequences **must** be present and have the same dimensions, bit depth and number of images.
///
/// The output sequence name starts with the prefix "mCFA\_" and a number unless otherwise specified with **-prefixout=** option
///
#[derive(Builder)]
pub struct SeqmergeCfa {}

impl Command for SeqmergeCfa {
    fn name() -> &'static str {
        "seqmerge_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqmodasinh sequence -D= [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels] [-prefix=]
/// ```
///
/// Same command as MODASINH but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`modasinh <modasinh>`
///
#[derive(Builder)]
pub struct Seqmodasinh {}

impl Command for Seqmodasinh {
    fn name() -> &'static str {
        "seqmodasinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqmtf sequencename low mid high [channels] [-prefix=]
/// ```
///
/// Same command as MTF but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "mtf\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`mtf <mtf>`
///
#[derive(Builder)]
pub struct Seqmtf {}

impl Command for Seqmtf {
    fn name() -> &'static str {
        "seqmtf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqprofile sequence -from=x,y -to=x,y [-tri] [-cfa] [-arcsec] [-savedat] [-layer=] [-width=] [-spacing=] [ {-xaxis=wavelength | -xaxis=wavenumber } ] [{-wavenumber1= | -wavelength1=} -wn1at=x,y {-wavenumber2= | -wavelength2=} -wn2at=x,y] ["-title=My Plot"]
/// ```
///
/// Generates an intensity profile plot between 2 points in each image in the sequence. After the mandatory first argument stating the sequence to process, the other arguments are the same as for the **profile** command. If processing a sequence and it is desired to have the current image number and total number of images displayed in the format "My Sequence (1 / 5)", the given title should end with () (e.g. "My Sequence ()" and the numbers will be populated automatically)
///
#[derive(Builder)]
pub struct Seqprofile {}

impl Command for Seqprofile {
    fn name() -> &'static str {
        "seqprofile"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqpsf sequencename [channel] [{ -at=x,y | -wcs=ra,dec }] [-followstar]
/// ```
///
/// Same command as PSF but runs on sequences. This is similar to the one-star registration, except results can be used for photometry analysis rather than aligning images and the coordinates of the star can be provided by options.
/// This command is what is called internally by the menu that appears on right click in the image, with the PSF for the sequence entry. If registration data already exist for the sequence, they will can be used to shift the search window in each image. If there is no registration data and if there is significant shift between images in the sequence, the default settings will fail to find stars in the initial position of the search area.
/// The follow star option can then be activated with the argument **-followstar**.
///
/// Results will be displayed in the Plot tab, from which they can also be exported to a comma-separated values (CSV) file for external analysis.
///
/// When creating a light curve, the first star for which seqpsf has been run, marked 'V' in the display, will be considered as the variable star. All others are averaged to create a reference light curve subtracted to the light curve of the variable star.
///
/// Currently, in headless operation, the command prints some analysed data in the console, another command allows several stars to be analysed and plotted as a light curve: LIGHT_CURVE. Arguments are mandatory: the sequence name must be provided ("." may be used to indicate the currently loaded sequence) and when headless it is mandatory to provide the coordinates of the star, with -at= allowing coordinates in pixels to be provided for the target star of -wcs= allowing J2000 equatorial coordinates to be provided
///
/// Links: :ref:`psf <psf>`, :ref:`light_curve <light_curve>`
///
#[derive(Builder)]
pub struct Seqpsf {}

impl Command for Seqpsf {
    fn name() -> &'static str {
        "seqpsf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqplatesolve sequencename [image_center_coords] [-focal=] [-pixelsize=]
/// seqplatesolve sequencename ... [-downscale] [-order=] [-radius=] [-force] [-noreg] [-disto=]
/// seqplatesolve sequencename ... [-limitmag=[+-]] [-catalog=] [-nocrop] [-nocache]
/// seqplatesolve sequencename ... [-localasnet [-blindpos] [-blindres]]
/// ```
///
/// Plate solve a sequence. A new sequence will be created with the prefix "ps\_" if the input sequence is SER, otherwise, the images headers will be updated. In case of SER, providing the metadata is mandatory and the output sequence will be in the FITS cube format, as SER cannot store WCS data.
/// If WCS or other image metadata are erroneous or missing, arguments must be passed:
/// the approximate image center coordinates can be provided in decimal degrees or degree/hour minute second values (J2000 with colon separators), with right ascension and declination values separated by a comma or a space (not mandatory for astrometry.net).
/// focal length and pixel size can be passed with **-focal=** (in mm) and **-pixelsize=** (in microns), overriding values from images and settings. See also options to solve blindly with local Astrometry.net
///
/// For faster star detection in big images, downsampling the image is possible with **-downscale**.
/// The solve can account for distortions using SIP convention with polynomials up to order 5. Default value is taken form the astrometry preferences. This can be changed with the option **-order=** giving a value between 1 and 5.
/// When using Siril solver local catalogues or with local Astrometry.net, if the initial solve is not successful, the solver will search for a solution within a cone of radius specified with **-radius=** option. If no value is passed, the search radius is taken from the astrometry preferences. Siril near search can be disabled by passing a value of 0. (cannot be disabled for Astrometry.net).
/// Images already solved will be skipped by default. This can be disabled by passing the option **-force**.
/// Using this command will update registration data unless the option **-noreg** is passed.
/// You can save the current solution as a distortion file with the option **-disto=**.
///
/// Images can be either plate solved by Siril using a star catalogue and the global registration algorithm or by astrometry.net's local solve-field command (enabled with **-localasnet**).
///
/// **Siril platesolver options:**
/// The limit magnitude of stars used for plate solving is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The choice of the star catalog is automatic unless the **-catalog=** option is passed: if local catalogs are installed, they are used, otherwise the choice is based on the field of view and limit magnitude. If the option is passed, it forces the use of the remote catalog given in argument, with possible values: tycho2, nomad, gaia, ppmxl, brightstars, apass.
/// If the computed field of view is larger than 5 degrees, star detection will be bounded to a cropped area around the center of the image unless **-nocrop** option is passed.
/// When using online catalogues, a single catalogue extraction will be done for the entire sequence. If there is a lot of drift or different sampling, that may not succeed for all images. This can be disabled by passing the argument **-nocache**, in which case metadata from each image will be used (except for the forced values like center coordinates, pixel size and/or focal length).
///
/// **Astrometry.net solver options:**
/// Passing options **-blindpos** and/or **-blindres** enables to solve blindly for position and for resolution respectively. You can use these when solving an image with a completely unknown location and sampling
///
#[derive(Builder)]
pub struct Seqplatesolve {}

impl Command for Seqplatesolve {
    fn name() -> &'static str {
        "seqplatesolve"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqresample sequencename { -scale= | -width= | -height= } [-interp=] [-prefix=]
/// ```
///
/// Scales the sequence given in argument **sequencename**. Only selected images in the sequence are processed.
///
/// The scale factor is specified either by the **-scale=** argument or by setting the output width, height or maximum dimension using the **-width=**, **-height=** or **-maxdim=** options.
///
/// An interpolation method may be specified using the **-interp=** argument followed by one of the methods in the list **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}.. Clamping is applied for cubic and lanczos interpolation.
///
/// The output sequence name starts with the prefix "scaled\_" unless otherwise specified with **-prefix=** option
///
#[derive(Builder)]
pub struct Seqresample {}

impl Command for Seqresample {
    fn name() -> &'static str {
        "seqresample"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqrl sequencename [-loadpsf=] [-alpha=] [-iters=] [-stop=] [-gdstep=] [-tv] [-fh] [-mul]
/// ```
///
/// The same as the RL command, but applies to a sequence which must be specified as the first argument
///
/// Links: :ref:`rl <rl>`
///
#[derive(Builder)]
pub struct Seqrl {}

impl Command for Seqrl {
    fn name() -> &'static str {
        "seqrl"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqsb sequencename [-loadpsf=] [-alpha=] [-iters=]
/// ```
///
/// The same as the SB command, but applies to a sequence which must be specified as the first argument
///
/// Links: :ref:`sb <sb>`
///
#[derive(Builder)]
pub struct Seqsb {}

impl Command for Seqsb {
    fn name() -> &'static str {
        "seqsb"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqsplit_cfa sequencename [-prefix=]
/// ```
///
/// Same command as SPLIT_CFA but for the sequence **sequencename**.
///
/// The output sequences names start with the prefix "CFA\_" and a number unless otherwise specified with **-prefix=** option.
/// *Limitation:* the sequence always outputs a sequence of FITS files, no matter the type of input sequence
///
/// Links: :ref:`split_cfa <split_cfa>`
///
#[derive(Builder)]
pub struct SeqsplitCfa {}

impl Command for SeqsplitCfa {
    fn name() -> &'static str {
        "seqsplit_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqstarnet sequencename [-stretch] [-upscale] [-stride=value] [-nostarmask]
/// ```
///
/// This command calls `Starnet++ <https://www.starnetastro.com/>`__ to remove stars from the sequence **sequencename**. See STARNET
///
/// Links: :ref:`starnet <starnet>`
///
#[derive(Builder)]
pub struct Seqstarnet {}

impl Command for Seqstarnet {
    fn name() -> &'static str {
        "seqstarnet"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqstat sequencename output_file [option] [-cfa]
/// ```
///
/// Same command as STAT for sequence **sequencename**.
///
/// Data is saved as a csv file **output_file**.
/// The optional parameter defines the number of statistical values computed: **basic**, **main** (default) or **full** (more detailed but longer to compute).
/// \\t\ **basic** includes mean, median, sigma, bgnoise, min and max
/// \\t\ **main** includes basic with the addition of avgDev, MAD and the square root of BWMV
/// \\t\ **full** includes main with the addition of location and scale.
///
/// If **-cfa** is passed and the images are CFA, statistics are made on per-filter extractions
///
/// Links: :ref:`stat <stat>`
///
#[derive(Builder)]
pub struct Seqstat {}

impl Command for Seqstat {
    fn name() -> &'static str {
        "seqstat"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqsubsky sequencename { -rbf | degree } [-nodither] [-samples=20] [-tolerance=1.0] [-smooth=0.5] [-prefix=]
/// ```
///
/// Same command as SUBSKY but for the sequence **sequencename**.
/// Dithering, required for low dynamic gradients, can be disabled with **-nodither**. Note that the **-existing** option is not available for sequence background removal, as the frames of a sequence are not necessarily always aligned.
///
/// The output sequence name starts with the prefix "bkg\_" unless otherwise specified with **-prefix=** option. Only selected images in the sequence are processed
///
/// Links: :ref:`subsky <subsky>`
///
#[derive(Builder)]
pub struct Seqsubsky {}

impl Command for Seqsubsky {
    fn name() -> &'static str {
        "seqsubsky"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// seqtilt sequencename
/// ```
///
/// Same command as TILT but for the sequence **sequencename**. It generally gives better results
///
/// Links: :ref:`tilt <tilt>`
///
#[derive(Builder)]
pub struct Seqtilt {}

impl Command for Seqtilt {
    fn name() -> &'static str {
        "seqtilt"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// sequpdate_key sequencename key value [keycomment]
/// sequpdate_key sequencename -delete key
/// sequpdate_key sequencename -modify key newkey
/// sequpdate_key sequencename -comment comment
/// ```
///
/// Same command as UPDATE_KEY but for the sequence **sequencename**. However, this command won't work on SER sequence
///
/// Links: :ref:`update_key <update_key>`
///
#[derive(Builder)]
pub struct SequpdateKey {}

impl Command for SequpdateKey {
    fn name() -> &'static str {
        "sequpdate_key"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// wiener sequencename [-loadpsf=] [-alpha=]
/// ```
///
/// The same as the **WIENER** command, but applies to a sequence which must be specified as the first argument
///
/// Links: :ref:`wiener <wiener>`
///
#[derive(Builder)]
pub struct Seqwiener {}

impl Command for Seqwiener {
    fn name() -> &'static str {
        "seqwiener"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// set { -import=inifilepath | variable=value }
/// ```
///
/// Updates a setting value, using its variable name, with the given value, or a set of values using an existing ini file with **-import=** option.
/// See GET to get values or the list of variables
///
/// Links: :ref:`get <get>`
///
#[derive(Builder)]
pub struct Set {}

impl Command for Set {
    fn name() -> &'static str {
        "set"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// set16bits
/// ```
///
/// Forbids images to be saved with 32 bits per channel on processing, use 16 bits instead
///
#[derive(Builder)]
pub struct Set16bits {}

impl Command for Set16bits {
    fn name() -> &'static str {
        "set16bits"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// set32bits
/// ```
///
/// Allows images to be saved with 32 bits per channel on processing
///
#[derive(Builder)]
pub struct Set32bits {}

impl Command for Set32bits {
    fn name() -> &'static str {
        "set32bits"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// setcompress 0/1 [-type=] [q]
/// ```
///
/// Defines if images are compressed or not.
///
/// **0** means no compression while **1** enables compression.
/// If compression is enabled, the type must be explicitly written in the option **-type=** ("rice", "gzip1", "gzip2").
/// Associated to the compression, the quantization value must be within [0, 256] range.
///
/// For example, "setcompress 1 -type=rice 16" sets the rice compression with a quantization of 16
///
#[derive(Builder)]
pub struct Setcompress {}

impl Command for Setcompress {
    fn name() -> &'static str {
        "setcompress"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// setcpu number
/// ```
///
/// Defines the number of processing threads used for calculation.
///
/// Can be as high as the number of virtual threads existing on the system, which is the number of CPU cores or twice this number if hyperthreading (Intel HT) is available. The default value is the maximum number of threads available, so this should mostly be used to limit processing power. This is reset on every Siril run. See also SETMEM
///
/// Links: :ref:`setmem <setmem>`
///
#[derive(Builder)]
pub struct Setcpu {}

impl Command for Setcpu {
    fn name() -> &'static str {
        "setcpu"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// setext extension
/// ```
///
/// Sets the extension used and recognized by sequences.
///
/// The argument **extension** can be "fit", "fts" or "fits"
///
#[derive(Builder)]
pub struct Setext {}

impl Command for Setext {
    fn name() -> &'static str {
        "setext"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// setfindstar [reset] [-radius=] [-sigma=] [-roundness=] [-focal=] [-pixelsize=] [-convergence=] [ [-gaussian] | [-moffat] ] [-minbeta=] [-relax=on|off] [-minA=] [-maxA=] [-maxR=]
/// ```
///
/// Defines stars detection parameters for FINDSTAR and REGISTER commands.
///
/// Passing no parameter lists the current values.
/// Passing **reset** resets all values to defaults. You can then still pass values after this keyword.
///
/// Configurable values:
///
/// **-radius=** defines the radius of the initial search box and must be between 3 and 50.
/// **-sigma=** defines the threshold above noise and must be greater than or equal to 0.05.
/// **-roundness=** defines minimum star roundness and must between 0 and 0.95. **-maxR** allows an upper bound to roundness to be set, to visualize only the areas where stars are significantly elongated, do not change for registration.
/// **-minA** and **-maxA** define limits for the minimum and maximum amplitude of stars to keep, normalized between 0 and 1.
/// **-focal=** defines the focal length of the telescope.
/// **-pixelsize=** defines the pixel size of the sensor.
/// **-gaussian** and **-moffat** configure the solver model to be used (Gaussian is the default).
/// If Moffat is selected, **-minbeta=** defines the minimum value of beta for which candidate stars will be accepted and must be greater than or equal to 0.0 and less than 10.0.
/// **-convergence=** defines the number of iterations performed to fit PSF and should be set between 1 and 3 (more tolerant).
/// **-relax=** relaxes the checks that are done on star candidates to assess if they are stars or not, to allow objects not shaped like stars to still be accepted (off by default)
///
/// Links: :ref:`findstar <findstar>`, :ref:`register <register>`, :ref:`psf <psf>`
///
/// The threshold for star detection is computed as the median of the image (which
/// represents in general the background level) plus k times sigma, sigma being the
/// standard deviation of the image (which is a good indication of the noise
/// amplitude). If you have many stars in your images and a good signal/noise
/// ratio, it may be a good idea to increase this value to speed-up the detection
/// and false positives.
///
/// It is recommended to test the values used for a sequence with Siril's GUI,
/// available in the dynamic PSF toolbox from the analysis menu. It may improve
/// registration quality to increase the parameters, but it is also important to be
/// able to detect several tens of stars in each image.
///
#[derive(Builder)]
pub struct Setfindstar {}

impl Command for Setfindstar {
    fn name() -> &'static str {
        "setfindstar"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// setmem ratio
/// ```
///
/// Sets a new ratio of used memory on free memory.
///
/// **Ratio** value should be between 0.05 and 2, depending on other activities of the machine. A higher ratio should allow siril to process faster, but setting the ratio of used memory above 1 will require the use of on-disk memory, which is very slow and unrecommended, even sometimes not supported, leading to system crash. A fixed amount of memory can also be set in the generic settings, with SET, instead of a ratio
///
/// Links: :ref:`set <set>`
///
#[derive(Builder)]
pub struct Setmem {}

impl Command for Setmem {
    fn name() -> &'static str {
        "setmem"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// setphot [-inner=20] [-outer=30] [-aperture=10] [-dyn_ratio=4.0] [-gain=2.3] [-min_val=0] [-max_val=60000]
/// ```
///
/// Gets or sets photometry settings, mostly used by SEQPSF. If arguments are provided, they will update the settings. None are mandatory, any can be provided, default values are shown in the command's syntax. At the end of the command, the active configuration will be printed.
///
/// The Aperture size is dynamic unless it is forced. If so, the **aperture** value from the settings is used. If dynamic, the radius of the aperture is defined by the supplied dynamic ratio ("radius/half-FWHM").
/// Allowed values for the argument **-dyn_ratio** are in the range [1.0, 5.0]. A value outside this range will automatically set the aperture to the fixed value **-aperture**.
///
/// Gain is used only if not available from the FITS header
///
/// Links: :ref:`seqpsf <seqpsf>`
///
#[derive(Builder)]
pub struct Setphot {}

impl Command for Setphot {
    fn name() -> &'static str {
        "setphot"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// setref sequencename image_number
/// ```
///
/// Sets the reference image of the sequence given in first argument. **image_number** is the sequential number of the image in the sequence, not the number in the filename, starting at 1
///
#[derive(Builder)]
pub struct Setref {}

impl Command for Setref {
    fn name() -> &'static str {
        "setref"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// spcc [-limitmag=[+-]] [ { -monosensor= [ -rfilter= ] [-gfilter=] [-bfilter=] | -oscsensor= [-oscfilter=] [-osclpf=] } ] [-whiteref=] [ -narrowband [-rwl=] [-gwl=] [-bwl=] [-rbw=] [-gbw=] [-bbw=] ] [-bgtol=lower,upper] [ -atmos [-obsheight=] { [-pressure=] | [-slp=] } ]
/// ```
///
/// Run the Spectrophotometric Color Correction on the loaded platesolved image.
///
/// The limit magnitude of stars is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The star catalog used for SPCC is always Gaia DR3: by default the local Gaia DR3 xp_sampled catalog will be used if available but this can be overridden with **-catalog={gaia \| localgaia}**.
///
/// The names of sensors and filters can be specified using the following options: **-monosensor=**, **-rfilter=**, **-gfilter=**, **-bfilter=** or **-oscsensor=**, **-oscfilter=**, **-osclpf=**; the name of the white reference can be specified using the **-whiteref=** option. In all cases the name must be provided exactly as it is in the combo boxes in the SPCC tool. Note that sensor, filter and white reference names may contain spaces: in this case when using them as arguments to the **spcc** command, the entire argument must be enclosed in quotation marks, for example "-whiteref=Average Spiral Galaxy".
///
/// Narrowband mode can be selected using the argument **-narrowband**, in which case the previous filter arguments are ignored and NB filter wavelengths and bandwidths can be provided using **-rwl=**, **-rbw=**, **-gwl=**, **-gbw=**, **-bwl=** and **-bbw=**.
///
/// If one of the spectral data argument is omitted, the previously used value will be used.
///
/// Background reference outlier tolerance can be specified in sigma units using **-bgtol=lower,upper**: these default to -2.8 and +2.0.
///
/// Atmospheric correction can be applied by passing **-atmos**. In this case the following optional arguments apply: **-obsheight=** specifies the observer's height above sea level in metres (default 10), **-pressure=** specifies local atmospheric pressure at the observing site in hPa, or **-slp=** specifies sea-level atmospheric pressure in hPa (default pressure is 1013.25 hPa at sea level)
///
#[derive(Builder)]
pub struct Spcc {}

impl Command for Spcc {
    fn name() -> &'static str {
        "spcc"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// spcc_list { oscsensor | monosensor | redfilter | greenfilter | bluefilter | oscfilter | osclpf | whiteref }
/// ```
///
/// Print a list of SPCC names available for use to define sensors, filters or white references using the **spcc** command. This command requires an argument to set which list is printed: the options are **oscsensor**, **monosensor**, **redfilter**, **greenfilter**, **bluefilter**, **oscfilter**, **osclpf** or **whiteref**.
/// Note that sensor, filter and white reference names may contain spaces: in this case when using them as arguments to the **spcc** command, the entire argument must be enclosed in quotation marks, for example "-whiteref=Average Spiral Galaxy"
///
/// Links: :ref:`spcc <spcc>`
///
#[derive(Builder)]
pub struct SpccList {}

impl Command for SpccList {
    fn name() -> &'static str {
        "spcc_list"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// split file1 file2 file3 [-hsl | -hsv | -lab]
/// ```
///
/// Splits the loaded color image into three distinct files (one for each color) and saves them in **file1**.fit, **file2**.fit and **file3**.fit files. A last argument can optionally be supplied, **-hsl**, **-hsv** or **lab** to perform an HSL, HSV or CieLAB extraction. If no option are provided, the extraction is of RGB type, meaning no conversion is done
///
#[derive(Builder)]
pub struct Split {}

impl Command for Split {
    fn name() -> &'static str {
        "split"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// split_cfa
/// ```
///
/// Splits the loaded CFA image into four distinct files (one for each channel) and saves them in files
///
#[derive(Builder)]
pub struct SplitCfa {}

impl Command for SplitCfa {
    fn name() -> &'static str {
        "split_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// stack seqfilename
/// stack seqfilename { sum | min | max } [-output_norm] [-out=filename] [-maximize] [-upscale] [-32b]
/// stack seqfilename { med | median } [-nonorm, -norm=] [-fastnorm] [-rgb_equal] [-output_norm] [-out=filename] [-32b]
/// stack seqfilename { rej | mean } [rejection type] [sigma_low sigma_high]  [-rejmap[s]] [-nonorm, -norm=] [-fastnorm] [-overlap_norm] [-weight={noise|wfwhm|nbstars|nbstack}] [-feather=] [-rgb_equal] [-output_norm] [-out=filename] [-maximize] [-upscale] [-32b]
/// ```
///
/// Stacks the **sequencename** sequence, using options.
///
/// Rejection type:
/// The allowed types are: **sum**, **max**, **min**, **med** (or **median**) and **rej** (or **mean**). If no argument other than the sequence name is provided, sum stacking is assumed.
///
/// Stacking with rejection:
/// Types **rej** or **mean** require the use of additional arguments for rejection type and values. The rejection type is one of **n[one], p[ercentile], s[igma], m[edian], w[insorized], l[inear], g[eneralized], [m]a[d]** for Percentile, Sigma, Median, Winsorized, Linear-Fit, Generalized Extreme Studentized Deviate Test or k-MAD clipping. If omitted, the default Winsorized is used.
/// The **sigma low** and **sigma high** parameters of rejection are mandatory unless **none** is selected.
/// Optionally, rejection maps can be created, showing where pixels were rejected in one (**-rejmap**) or two (**-rejmaps**, for low and high rejections) newly created images.
///
/// Normalization of input images:
/// For **med** (or **median**) and **rej** (or **mean**) stacking types, different types of normalization are allowed: **-norm=add** for additive, **-norm=mul** for multiplicative. Options **-norm=addscale** and **-norm=mulscale** apply same normalization but with scale operations. **-nonorm** is the option to disable normalization. Otherwise addtive with scale method is applied by default.
/// **-fastnorm** option specifies to use faster estimators for location and scale than the default IKSS.
/// **-overlap_norm**, if passed, will compute normalization coeffcients on images overlaps instead of whole images (allowed only if **-maximize** is passed).
///
/// Other options for rejection stacking:
/// Weighting can be applied to the images of the sequences using the option **-weight=** followed by:
/// **noise** to add larger weights to frames with lower background noise.
/// **nbstack** to weight input images based on how many images were used to create them, useful for live stacking.
/// **nbstars** or **wfwhm** to weight input images based on number of stars or wFWHM computed during registration step.
/// **-feather=** option will apply a feathering mask on each image borders over the distance (in pixels) given in argument.
///
/// Outputs:
/// Result image name can be set with the **-out=** option. Otherwise, it will be named as **sequencename**\ \_stacked.fit.
/// **-output_norm** applies a normalization to rescale result in the [0, 1] range (median and mean stacking only).
/// **-maximize** option will use registration data from the sequence to create a stacked image that encompasses all the images of the sequence (applicable to all methods except median stacking).
/// **-upscale** option will upscale the sequence by a factor 2 prior to stacking using the registration data (applicable to all methods except median stacking).
/// **-rgb_equal** will use normalization to equalize color image backgrounds, useful if PCC/SPCC or unlinked AUTOSTRETCH will not be used.
/// **-32b** will override the bitdepth set in Preferences and save the stacked image in 32b.
///
///
/// Filtering out images:
/// Images to be stacked can be selected based on some filters, like manual selection or best FWHM, with some of the **-filter-\*** options.
///
///
/// Links: :ref:`pcc <pcc>`, :ref:`spcc <spcc>`, :ref:`autostretch <autostretch>`
///
/// ```text
/// [-filter-fwhm=value[%|k]] [-filter-wfwhm=value[%|k]] [-filter-round=value[%|k]] [-filter-bkg=value[%|k]]
/// [-filter-nbstars=value[%|k]] [-filter-quality=value[%|k]] [-filter-incl[uded]]
/// ```
/// Best images from the sequence can be stacked by using the filtering arguments. Each of these arguments can remove bad images based on a property their name contains, taken from the registration data, with either of the three types of argument values:
/// - a numeric value for the worse image to keep depending on the type of data used (between 0 and 1 for roundness and quality, absolute values otherwise),
/// - a percentage of best images to keep if the number is followed by a % sign,
/// - or a k value for the k.sigma of the worse image to keep if the number is followed by a k sign.
/// It is also possible to use manually selected images, either previously from the GUI or with the select or unselect commands, using the **-filter-included** argument.
///
#[derive(Builder)]
pub struct Stack {}

impl Command for Stack {
    fn name() -> &'static str {
        "stack"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// stackall
/// stackall { sum | min | max } [-maximize] [-upscale] [-32b]
/// stackall { med | median } [-nonorm, norm=] [-32b]
/// stackall { rej | mean } [rejection type] [sigma_low sigma_high] [-nonorm, norm=] [-overlap_norm] [-weight={noise|wfwhm|nbstars|nbstack}] [-feather=] [-rgb_equal] [-out=filename] [-maximize] [-upscale] [-32b]
/// ```
///
/// Opens all sequences in the current directory and stacks them with the optionally specified stacking type and filtering or with sum stacking. See STACK command for options description
///
/// Links: :ref:`stack <stack>`
///
#[derive(Builder)]
pub struct Stackall {}

impl Command for Stackall {
    fn name() -> &'static str {
        "stackall"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// starnet [-stretch] [-upscale] [-stride=value] [-nostarmask]
/// ```
///
/// Calls `StarNet <https://www.starnetastro.com/>`__ to remove stars from the loaded image.
///
/// **Prerequisite:** StarNet is an external program, with no affiliation with Siril, and must be installed correctly prior the first use of this command, with the path to its CLI version installation correctly set in Preferences / Miscellaneous.
///
/// The starless image is loaded on completion, and a star mask image is created in the working directory unless the optional parameter **-nostarmask** is provided.
///
/// Optionally, parameters may be passed to the command:
/// - The option **-stretch** is for use with linear images and will apply a pre-stretch before running StarNet and the inverse stretch to the generated starless and starmask images.
/// - To improve star removal on images with very tight stars, the parameter **-upscale** may be provided. This will upsample the image by a factor of 2 prior to StarNet processing and rescale it to the original size afterwards, at the expense of more processing time.
/// - The optional parameter **-stride=value** may be provided, however the author of StarNet *strongly* recommends that the default stride of 256 be used
///
#[derive(Builder)]
pub struct Starnet {}

impl Command for Starnet {
    fn name() -> &'static str {
        "starnet"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// start_ls [-dark=filename] [-flat=filename] [-rotate] [-32bits]
/// ```
///
/// Initializes a livestacking session, using the optional calibration files and waits for input files to be provided by the LIVESTACK command until STOP_LS is called. Default processing will use shift-only registration and 16-bit processing because it's faster, it can be changed to rotation with **-rotate** and **-32bits**
///
/// *Note that the live stacking commands put Siril in a state in which it's not able to process other commands. After START_LS, only LIVESTACK, STOP_LS and EXIT can be called until STOP_LS is called to return Siril in its normal, non-live-stacking, state*
///
/// Links: :ref:`livestack <livestack>`, :ref:`stop_ls <stop_ls>`, :ref:`exit <exit>`
///
#[derive(Builder)]
pub struct StartLs {}

impl Command for StartLs {
    fn name() -> &'static str {
        "start_ls"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// stat [-cfa] [main]
/// ```
///
/// Returns statistics of the current image, the basic list by default or the main list if **main** is passed. If a selection is made, statistics are computed within the selection. If **-cfa** is passed and the image is CFA, statistics are made on per-filter extractions
///
#[derive(Builder)]
pub struct Stat {}

impl Command for Stat {
    fn name() -> &'static str {
        "stat"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// stop_ls
/// ```
///
/// Stops the live stacking session. Only possible after START_LS
///
/// Links: :ref:`start_ls <start_ls>`
///
#[derive(Builder)]
pub struct StopLs {}

impl Command for StopLs {
    fn name() -> &'static str {
        "stop_ls"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// subsky { -rbf | degree } [-dither] [-samples=20] [-tolerance=1.0] [-smooth=0.5] [-existing]
/// ```
///
/// Computes a synthetic background gradient using either the polynomial function model of **degree** degrees or the RBF model (if **-rbf** is provided instead) and subtracts it from the image.
/// The number of samples per horizontal line and the tolerance to exclude brighter areas can be adjusted with the optional arguments. Tolerance is in MAD units: median + tolerance \* mad.
/// Dithering, required for low dynamic gradients, can be enabled with **-dither**.
/// For RBF, the additional smoothing parameter is also available. To use pre-existing background samples (e.g. if you have set background samples using a Python script) the **-existing** argument must be used
///
#[derive(Builder)]
pub struct Subsky {}

impl Command for Subsky {
    fn name() -> &'static str {
        "subsky"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// synthstar
/// ```
///
/// Fixes imperfect stars from the loaded image. No matter how much coma, tracking drift or other distortion your stars have, if Siril's star finder routine can detect it, synthstar will fix it. To use intensive care, you may wish to manually detect all the stars you wish to fix. This can be done using the findstar console command or the Dynamic PSF dialog. If you have not run star detection, it will be run automatically with default settings.
///
/// For best results synthstar should be run before stretching.
///
/// The output of synthstar is a fully corrected synthetic star mask comprising perfectly round star PSFs (Moffat or Gaussian profiles depending on star saturation) computed to match the intensity, FWHM, hue and saturation measured for each star detected in the input image. This can then be recombined with the starless image to produce an image with perfect stars.
///
/// No parameters are required for this command
///
/// Links: :ref:`psf <psf>`
///
#[derive(Builder)]
pub struct Synthstar {}

impl Command for Synthstar {
    fn name() -> &'static str {
        "synthstar"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// threshlo level
/// ```
///
/// Replaces values below **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Threshlo {}

impl Command for Threshlo {
    fn name() -> &'static str {
        "threshlo"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// threshi level
/// ```
///
/// Replaces values above **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Threshhi {}

impl Command for Threshhi {
    fn name() -> &'static str {
        "threshhi"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// thresh lo hi
/// ```
///
/// Replaces values below **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Thresh {}

impl Command for Thresh {
    fn name() -> &'static str {
        "thresh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// trixel [-p]
/// ```
///
/// For developers.
///
/// Without any argument, lists all the trixels of level 3 visible in the plate-solved image. The stars from each trixel can then be shown with command CONESEARCH using **-trix=** followed by a visible trixel number
///
/// With argument **-p**, prints out all the valid stars from all the 512 level3 trixels to file "trixels.csv"
///
/// Links: :ref:`conesearch <conesearch>`
///
#[derive(Builder)]
pub struct Trixel {}

impl Command for Trixel {
    fn name() -> &'static str {
        "trixel"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// unclipstars
/// ```
///
/// Re-profiles clipped stars of the loaded image to desaturate them, scaling the output so that all pixel values are <= 1.0
///
#[derive(Builder)]
pub struct Unclipstars {}

impl Command for Unclipstars {
    fn name() -> &'static str {
        "unclipstars"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// unpurple [-starmask] [-blue=value] [-thresh=value]
/// ```
///
/// Applies a cosmetic filter to reduce effects of purple fringing on stars.
///
/// If the **-starmask** parameter is given, a star mask will be used to identify areas of the image to affect. If a Dynamic PSF has already been run, this will be used for the starmask, otherwise one will be created automatically. The **-mod=** parameter should be given a value somewhere around 0.14 to reduce the amount of purple. The **-thresh=** will specify the size modifier for each star in the starmask and should be large enough to cause the stars to be entirely processed without remaining purple fringing. The value should between 0 and 1, typically around 0.5.
/// If the **-starmask** parameter is not given, the purple reduction will be applied across the entire image for any purple pixels with a luminance value higher than the given **-thresh=**. In this case, the **-thresh=** value should be reasonably low. This mode is useful for starmasks or other images without nebula or galaxy
///
/// Links: :ref:`psf <psf>`
///
#[derive(Builder)]
pub struct Unpurple {}

impl Command for Unpurple {
    fn name() -> &'static str {
        "unpurple"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// unselect sequencename from to
/// ```
///
/// Allows easy mass unselection of images in the sequence **sequencename** (from **from** to **to** included). See SELECT
///
/// Links: :ref:`select <select>`
///
#[derive(Builder)]
pub struct Unselect {}

impl Command for Unselect {
    fn name() -> &'static str {
        "unselect"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// unsharp sigma multi
/// ```
///
/// Applies an unsharp mask, actually a Gaussian filtered image with sigma **sigma** and a blend with the parameter **amount** used as such: out = in \* (1 + amount) + filtered \* (-amount).
///
/// See also GAUSS, the same without blending
///
/// Links: :ref:`gauss <gauss>`
///
#[derive(Builder)]
pub struct Unsharp {}

impl Command for Unsharp {
    fn name() -> &'static str {
        "unsharp"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// update_key key value [keycomment]
/// update_key -delete key
/// update_key -modify key newkey
/// update_key -comment comment
/// ```
///
/// Updates FITS keyword. Please note that the validity of **value** is not checked. This verification is the responsibility of the user. It is also possible to delete a key with the **-delete** option in front of the name of the key to be deleted, or to modify the key with the **-modify** option. The latter must be followed by the key to be modified and the new key name. Finally, the **-comment** option, followed by text, adds a comment to the FITS header. Please note that any text containing spaces must be enclosed in double quotation marks
///
#[derive(Builder)]
pub struct UpdateKey {}

impl Command for UpdateKey {
    fn name() -> &'static str {
        "update_key"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// wavelet nbr_layers type
/// ```
///
/// Computes the wavelet transform of the loaded image on (**nbr_layers**\ =1...n) layer(s) using linear (**type**\ =1) or bspline (**type**\ =2) version of the 'à trous' algorithm. The result is stored in a file as a structure containing the layers, ready for weighted reconstruction with WRECONS.
///
/// See also EXTRACT
///
/// Links: :ref:`wrecons <wrecons>`, :ref:`extract <extract>`
///
#[derive(Builder)]
pub struct Wavelet {}

impl Command for Wavelet {
    fn name() -> &'static str {
        "wavelet"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// wiener [-loadpsf=] [-alpha=]
/// ```
///
/// Restores an image using the Wiener deconvolution method.
///
/// Optionally, a PSF created by MAKEPSF may be loaded using the argument **-loadpsf=\ filename**.
///
/// The parameter **-alpha=** provides the Gaussian noise modelled regularization factor
///
/// Links: :ref:`psf <psf>`, :ref:`makepsf <makepsf>`
///
#[derive(Builder)]
pub struct Wiener {}

impl Command for Wiener {
    fn name() -> &'static str {
        "wiener"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}


/// ```text
/// wrecons c1 c2 c3 ...
/// ```
///
/// Reconstructs to current image from the layers previously computed with wavelets and weighted with coefficients **c1**, **c2**, ..., **cn** according to the number of layers used for wavelet transform, after the use of WAVELET
///
/// Links: :ref:`wavelet <wavelet>`
///
#[derive(Builder)]
pub struct Wrecons {}

impl Command for Wrecons {
    fn name() -> &'static str {
        "wrecons"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
