/* When a post's ID is clicked, add a reference to that post in the reply box
 * at the top of the page. */
function onClickPostId(ev) {
    ev.preventDefault()

    var id = ev.target.textContent.replace("#", "")
    var reference = ">>" + id + "\n\n"

    var textarea = document.querySelector('.new-item-form textarea')

    textarea.value += reference
    textarea.focus()
}

document.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('.post .post-id').forEach((elem) => {
        elem.addEventListener('click', onClickPostId)
    })
})
