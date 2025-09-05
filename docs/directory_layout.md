# Directory Layout

Based on a lot of experience from other imagers the best layout of your directories for organization and Photonyx looks like this:

```
{astro-root-directory}
|-- {telescope-profile-home}
|--|-- px_profile.yaml
|--|-- BIAS
|--|--|-- {date-without-time}
|--|--|--|-- RAW
|--|--|--|-- BIAS_{date-without-time}_stacked.{ext}
|--|-- DARK
|--|--|-- {date-without-time}
|--|--|--|-- RAW_{exposure}_{temperature}
|--|--|--|-- DARK_{date-without-time}_{exposure}_{temperature}_stacked.{ext}
|--|-- FLAT
|--|--|-- {date-without-time}
|--|--|--|-- RAW_{filter-key}
|--|--|--|-- PP_{filter-key}
|--|--|--|-- FLAT_{date-without-time}_{filter-key}_stacked.{ext}
|--|-- LIGHT
|--|--|-- {target}
|--|--|--|-- {date-without-time}
|--|--|--|--|-- RAW_{exposure}_{filter-key}_{temperature}
|--|--|--|--|-- PP_{exposure}_{filter-key}_{temperature}
|--|--|--|--|-- px_session.yaml
|--|-- PROJECTS
|--|--|-- {project-name}
|--|--|--|-- {filter-key}_linear_stacked.{fit}
```

## Gaps to incorporate
* [ ] Different binning
* [ ] Mosiacs (diff target layout?)
* [ ] Rotation with rotators (changes calibration used)
* [ ] Different Gain or Offsets (changes calibration used)


# NINA

In the Imaging options for Nina this would be the pattern that would capture into this structure:

`$$TELESCOPE$$\$$IMAGETYPE$$\$$TARGETNAME$$\$$DATEMINUS12$$\RAW_$$FILTER$$_$$EXPOSURETIME$$\$$DATETIME$$_$$FILTER$$_$$SENSORTEMP$$c_$$GAIN$$g_$$OFFSET$$o_$$EXPOSURETIME$$s_$$ROTATORANGLE$$d_$$FRAMENR$$`