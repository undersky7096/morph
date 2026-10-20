import mark from "./assets/mark.png";

/** Logo de Morph : le M sur un carré arrondi, aux couleurs de l'apparence (comme l'icône). */
export default function Logo({ className = "" }: { className?: string }) {
  return (
    <span className={`logo ${className}`} aria-hidden>
      <span className="logo-mark" style={{ maskImage: `url(${mark})`, WebkitMaskImage: `url(${mark})` }} />
    </span>
  );
}
