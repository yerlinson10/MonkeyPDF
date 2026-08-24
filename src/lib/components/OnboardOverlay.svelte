<script lang="ts">
  /* Hallmark · component: onboard-overlay · genre: playful-técnico · theme: press-shop banana
   * states: default · hover · focus · active · disabled · loading · error · success
   * contrast: pass
   */
  let { ondone }: { ondone: () => void } = $props()
</script>

<div class="mp-onboard" role="dialog" aria-labelledby="mp-onboard-title" aria-modal="true">
  <div class="mp-onboard-sheet">
    <div class="mp-hero-mark" aria-hidden="true">
      <div class="badge">
        <img src="/favicon.svg" alt="" width="44" height="44" />
      </div>
    </div>
    <p class="kicker">Instalado</p>
    <h2 id="mp-onboard-title">Hazlo local.</h2>
    <p>
      23 herramientas en el rail. Un clic. El PDF no sale de tu máquina. Ctrl+K busca, Esc cierra,
      Ctrl+Enter ejecuta.
    </p>
    <div class="mp-onboard-actions">
      <button type="button" class="mp-btn mp-btn-primary" onclick={ondone}>Empezar a sellar</button>
    </div>
  </div>
</div>
