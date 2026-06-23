import logoUrl from '../assets/notefix-logo.svg';

interface Props {
  size?: number;
  className?: string;
}

export default function Logo({ size = 20, className }: Props) {
  return <img src={logoUrl} alt="Notefix" width={size} height={size} className={className} />;
}
