<script lang="ts">
  // Clin d'œil du jour (#399) : un petit dessin posé sur le nom de l'app, dans l'en-tête.
  // Placé dans `.brand`, qu'il recouvre : les positions sont en pourcentage de sa largeur.
  // Aucune animation en boucle ; celles qui jouent une fois disparaissent avec « réduire
  // les animations ».
  import { eggs } from "../lib/eggs/eggs.svelte";
</script>

{#if eggs.date}
  <span class="egg-mark {eggs.date}" aria-hidden="true">
    {#if eggs.date === "foundation"}
      <!-- Anniversaire du premier commit : une bougie. -->
      <svg class="on-s" viewBox="0 0 10 16" width="8" height="13">
        <path d="M5 1c1.6 2 1.6 3.4 0 4.6C3.4 4.4 3.4 3 5 1z" fill="#ffb547" />
        <rect x="3.5" y="6" width="3" height="10" rx="0.8" fill="currentColor" />
      </svg>
    {:else if eggs.date === "mozart"}
      <!-- 27 janvier : une arabesque de partition sous le nom. -->
      <svg class="under" viewBox="0 0 100 10" preserveAspectRatio="none">
        <path
          d="M2 6c10-6 14 6 24 0s14-6 24 0 14 6 24 0 14-6 24 0"
          fill="none"
          stroke="currentColor"
          stroke-width="1.2"
          stroke-linecap="round"
        />
      </svg>
    {:else if eggs.date === "lv426"}
      <!-- 26 avril : un point de détecteur de mouvement pulse une fois, puis s'efface. -->
      <span class="blip"></span>
    {:else if eggs.date === "may4"}
      <!-- 4 mai : deux petits soleils se couchent derrière le nom. -->
      <svg class="behind" viewBox="0 0 40 14" width="34" height="12">
        <circle cx="13" cy="10" r="5" fill="#ffb547" opacity="0.75" />
        <circle cx="25" cy="11" r="3.4" fill="#ff7a45" opacity="0.75" />
      </svg>
    {:else if eggs.date === "towel"}
      <!-- 25 mai : une serviette posée sur le nom. -->
      <svg class="on-s" viewBox="0 0 16 12" width="14" height="10">
        <path d="M1 3h14v4l-2 4H3L1 7z" fill="#e8e1d0" />
        <path d="M1 5h14M2 8h12" stroke="#7c9cff" stroke-width="1" />
      </svg>
    {:else if eggs.date === "musicDay"}
      <!-- 21 juin : une note de musique sur le nom. -->
      <svg class="on-end" viewBox="0 0 12 16" width="10" height="13">
        <path d="M8 1v10.2A2.6 2.6 0 1 1 6.6 9V1h1.4zM8 1c1.4 1.5 3.5 2 3.5 4.4" fill="none" stroke="currentColor" stroke-width="1.4" />
      </svg>
    {:else if eggs.date === "swissDay"}
      <!-- 1er août : des feux sur une ligne de crêtes, un lampion pendu au nom. -->
      <svg class="ridge" viewBox="0 0 60 16" width="54" height="14">
        <path d="M0 15 10 6l6 5 9-9 8 8 7-5 10 9 10-6" fill="none" stroke="currentColor" stroke-width="1" opacity="0.6" />
        <circle cx="10" cy="5" r="1.4" fill="#ff7a45" />
        <circle cx="25" cy="1.6" r="1.4" fill="#ffb547" />
        <circle cx="40" cy="4" r="1.4" fill="#ff7a45" />
      </svg>
      <svg class="lantern" viewBox="0 0 10 18" width="8" height="15">
        <path d="M5 0v5" stroke="currentColor" stroke-width="0.8" />
        <ellipse cx="5" cy="11" rx="4.4" ry="5.6" fill="#e23b3b" />
        <path d="M5 8.2v5.6M2.4 11h5.2" stroke="#fff" stroke-width="1.4" />
      </svg>
    {:else if eggs.date === "hobbits"}
      <!-- 22 septembre : un petit feu d'artifice part du nom, une fois. -->
      <svg class="burst" viewBox="0 0 24 24" width="22" height="22">
        {#each [0, 45, 90, 135, 180, 225, 270, 315] as angle (angle)}
          <path d="M12 7V2" stroke={angle % 90 ? "#ffb547" : "#ff5c8a"} stroke-width="1.4" stroke-linecap="round" transform="rotate({angle} 12 12)" />
        {/each}
      </svg>
    {:else if eggs.date === "halloween"}
      <!-- 31 octobre : une petite citrouille découpée. -->
      <svg class="on-s" viewBox="0 0 14 13" width="12" height="11">
        <path d="M7 3c4.6-.4 6.5 2 6.5 4.8S11 12.6 7 12.6.5 10.6.5 7.8 2.4 2.6 7 3z" fill="#ff7a1a" />
        <path d="M7 3V.8" stroke="#3ddc97" stroke-width="1.4" />
        <path d="M3.6 6.6 5 8 3.6 8zM10.4 6.6 9 8h1.4zM4.4 9.8l1.2.8 1.4-.8 1.4.8 1.2-.8" fill="#1a0f00" stroke="#1a0f00" stroke-width="0.6" />
      </svg>
    {:else if eggs.date === "cecilia"}
      <!-- 22 novembre : une petite auréole. -->
      <svg class="on-s halo" viewBox="0 0 16 6" width="14" height="5">
        <ellipse cx="8" cy="3" rx="6.6" ry="2" fill="none" stroke="#ffd36b" stroke-width="1.3" />
      </svg>
    {:else if eggs.date === "christmas"}
      <!-- 24 et 25 décembre : un peu de neige sur le nom. -->
      <svg class="cap" viewBox="0 0 100 8" preserveAspectRatio="none">
        <path d="M2 7C4 2 9 2 12 5s6-4 12-2 4 4 10 1 8-3 12 0 6 3 11-1 9 0 12 3 5-3 10-2 6 4 10 2" fill="#fff" stroke="#fff" stroke-width="2" stroke-linejoin="round" />
      </svg>
    {/if}
  </span>
{/if}

<style>
  .egg-mark {
    position: absolute;
    inset: 0;
    pointer-events: none;
    /* Sous le texte : la couleur de l'en-tête, pas celle du nom. */
    color: var(--text-muted);
  }

  svg,
  .blip {
    position: absolute;
    display: block;
  }

  /* Posé sur la première lettre. */
  .on-s {
    left: -2px;
    top: -9px;
  }

  .halo {
    top: -6px;
  }

  .on-end {
    right: -10px;
    top: -10px;
  }

  .under {
    left: 0;
    right: 0;
    bottom: -6px;
    width: 100%;
    height: 6px;
  }

  .cap {
    left: 0;
    top: -3px;
    width: 100%;
    height: 6px;
    opacity: 0.9;
  }

  .behind {
    right: -14px;
    bottom: 1px;
    z-index: -1;
  }

  .ridge {
    left: calc(100% + 54px);
    bottom: 0;
  }

  .lantern {
    right: -6px;
    top: 72%;
  }

  .burst {
    right: -18px;
    top: -14px;
    opacity: 0;
    animation: egg-burst 1.6s ease-out 0.6s 1;
  }

  @keyframes egg-burst {
    0% {
      opacity: 0;
      transform: scale(0.2);
    }
    30% {
      opacity: 1;
    }
    100% {
      opacity: 0;
      transform: scale(1.2);
    }
  }

  .blip {
    left: calc(100% + 60px);
    top: 50%;
    width: 6px;
    height: 6px;
    margin-top: -3px;
    border-radius: 50%;
    background: #3ddc97;
    box-shadow: 0 0 6px #3ddc97;
    opacity: 0;
    animation: egg-blip 2.4s ease-out 1s 1;
  }

  @keyframes egg-blip {
    0% {
      opacity: 0;
      transform: scale(0.4);
    }
    20% {
      opacity: 1;
      transform: scale(1);
    }
    100% {
      opacity: 0;
      transform: scale(2.2);
    }
  }

  /* Android : nom centré et version dessous, la crête et le point n'ont pas la place. */
  :global(:root[data-platform="android"]) .ridge,
  :global(:root[data-platform="android"]) .blip {
    display: none;
  }

  :global(:root[data-motion="reduced"]) .burst,
  :global(:root[data-motion="reduced"]) .blip {
    display: none;
  }
</style>
