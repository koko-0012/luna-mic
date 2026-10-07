import genericMicrophone from '../assets/microphones/generic/microphone.svg';
import profiles from '../assets/microphones/profiles.json';

type ArtworkRule = { matches: string[]; image?: string | null };
type Profile = ArtworkRule & { brand: string; models?: ArtworkRule[] };

// Only bundled assets can be selected. No downloads, remote URLs or arbitrary
// filesystem paths come from device names or profile data.
const images = import.meta.glob<string>('../assets/microphones/**/*.{svg,png,webp}', {
  eager: true,
  query: '?url',
  import: 'default',
});

export function microphoneArtwork(name: string, brand: string): string {
  const normalizedName = name.toLowerCase();
  const profile = (profiles as Profile[]).find((profile) => profile.brand === brand);
  const model = profile?.models?.find((rule) =>
    rule.matches.some((match) => normalizedName.includes(match.toLowerCase())),
  );
  // A recognized model without approved artwork stays generic. Do not display
  // another product's photo just because it belongs to the same manufacturer.
  const path = model ? model.image : profile?.image;
  return (path && images[`../assets/microphones/${path}`]) || genericMicrophone;
}

export { genericMicrophone };
